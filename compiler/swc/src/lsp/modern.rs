use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};
use sw_common::{Severity, Span};
use sw_semantic::{SymbolOccurrence, analyze_with_source};

#[derive(Default)]
struct LspServer {
    documents: HashMap<String, Document>,
}

struct Document {
    text: String,
    version: i64,
}

#[derive(Deserialize)]
struct Request {
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Deserialize)]
struct TextDocument {
    uri: String,
    #[serde(default)]
    version: i64,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct DidOpenParams {
    #[serde(rename = "textDocument")]
    text_document: TextDocument,
}

#[derive(Deserialize)]
struct DidCloseParams {
    #[serde(rename = "textDocument")]
    text_document: TextDocument,
}

#[derive(Deserialize)]
struct Position {
    line: u64,
    character: u64,
}

#[derive(Deserialize)]
struct Range {
    start: Position,
    end: Position,
}

#[derive(Deserialize)]
struct ContentChange {
    #[serde(default)]
    range: Option<Range>,
    text: String,
}

#[derive(Deserialize)]
struct DidChangeParams {
    #[serde(rename = "textDocument")]
    text_document: TextDocument,
    #[serde(rename = "contentChanges")]
    content_changes: Vec<ContentChange>,
}

#[derive(Deserialize)]
struct TextDocumentPositionParams {
    #[serde(rename = "textDocument")]
    text_document: TextDocument,
    position: Position,
}

impl LspServer {
    fn handle(&mut self, request: Request) -> Vec<Value> {
        match request.method.as_str() {
            "initialize" => vec![response(
                request.id,
                json!({
                    "capabilities": {
                        "textDocumentSync": { "openClose": true, "change": 2 },
                        "hoverProvider": true,
                        "definitionProvider": true
                    },
                    "serverInfo": { "name": "swc-lsp", "version": env!("CARGO_PKG_VERSION") }
                }),
            )],
            "shutdown" => request
                .id
                .into_iter()
                .map(|id| response(Some(id), Value::Null))
                .collect(),
            "textDocument/didOpen" => self.did_open(request.params),
            "textDocument/didChange" => self.did_change(request.params),
            "textDocument/didClose" => self.did_close(request.params),
            "textDocument/hover" => match request.id {
                Some(id) => vec![response(Some(id), self.hover(request.params))],
                None => Vec::new(),
            },
            "textDocument/definition" => match request.id {
                Some(id) => vec![response(Some(id), self.definition(request.params))],
                None => Vec::new(),
            },
            _ => request
                .id
                .into_iter()
                .map(|id| response(Some(id), Value::Null))
                .collect(),
        }
    }

    fn did_open(&mut self, params: Value) -> Vec<Value> {
        let Ok(params) = serde_json::from_value::<DidOpenParams>(params) else {
            return Vec::new();
        };
        let document = params.text_document;
        let uri = document.uri.clone();
        self.documents.insert(
            uri.clone(),
            Document {
                text: document.text,
                version: document.version,
            },
        );
        vec![self.publish(&uri)]
    }

    fn did_change(&mut self, params: Value) -> Vec<Value> {
        let Ok(params) = serde_json::from_value::<DidChangeParams>(params) else {
            return Vec::new();
        };
        let uri = params.text_document.uri;
        let document = self.documents.entry(uri.clone()).or_insert(Document {
            text: String::new(),
            version: params.text_document.version,
        });
        for change in params.content_changes {
            if let Some(range) = change.range {
                apply_change(&mut document.text, &range, &change.text);
            } else {
                document.text = change.text;
            }
        }
        document.version = params.text_document.version;
        vec![self.publish(&uri)]
    }

    fn did_close(&mut self, params: Value) -> Vec<Value> {
        let Ok(params) = serde_json::from_value::<DidCloseParams>(params) else {
            return Vec::new();
        };
        let uri = params.text_document.uri;
        self.documents.remove(&uri);
        vec![json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": { "uri": uri, "diagnostics": [] }
        })]
    }

    fn publish(&self, uri: &str) -> Value {
        let Some(document) = self.documents.get(uri) else {
            return Value::Null;
        };
        let (diagnostics, _) = self.analyze(uri, &document.text);
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": { "uri": uri, "version": document.version, "diagnostics": diagnostics }
        })
    }

    fn hover(&self, params: Value) -> Value {
        let Ok(params) = serde_json::from_value::<TextDocumentPositionParams>(params) else {
            return Value::Null;
        };
        let Some(document) = self.documents.get(&params.text_document.uri) else {
            return Value::Null;
        };
        let (_, symbols) = self.analyze(&params.text_document.uri, &document.text);
        let Some(symbol) = find_symbol(&symbols, &document.text, &params.position) else {
            return Value::Null;
        };
        json!({
            "contents": { "kind": "markdown", "value": format!("```sw\n{}\n```", symbol.detail) },
            "range": span_to_range(&document.text, symbol.span)
        })
    }

    fn definition(&self, params: Value) -> Value {
        let Ok(params) = serde_json::from_value::<TextDocumentPositionParams>(params) else {
            return Value::Null;
        };
        let Some(document) = self.documents.get(&params.text_document.uri) else {
            return Value::Null;
        };
        let (_, symbols) = self.analyze(&params.text_document.uri, &document.text);
        let Some(symbol) = find_symbol(&symbols, &document.text, &params.position) else {
            return Value::Null;
        };
        let definition_text = if path_matches(
            &uri_to_path(&params.text_document.uri),
            &symbol.definition_file,
        ) {
            document.text.clone()
        } else {
            std::fs::read_to_string(&symbol.definition_file).unwrap_or_default()
        };
        json!({
            "uri": path_to_uri(&symbol.definition_file),
            "range": span_to_range(&definition_text, symbol.definition_span)
        })
    }

    fn analyze(&self, uri: &str, text: &str) -> (Vec<Value>, Vec<SymbolOccurrence>) {
        let path = uri_to_path(uri);
        let result = analyze_with_source(&path, None, Some(text.to_string()));
        let diagnostics = result
            .diagnostics
            .items
            .iter()
            .filter(|item| {
                item.file
                    .as_ref()
                    .is_none_or(|file| path_matches(file, &path))
            })
            .map(|item| {
                let range = item
                    .span
                    .map(|span| span_to_range(text, span))
                    .unwrap_or_else(zero_range);
                json!({
                    "range": range,
                    "severity": if item.severity == Severity::Error { 1 } else { 2 },
                    "source": "swc",
                    "message": item.message,
                })
            })
            .collect();
        (diagnostics, result.symbol_occurrences)
    }
}

fn response(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": result })
}

fn find_symbol<'a>(
    symbols: &'a [SymbolOccurrence],
    text: &str,
    position: &Position,
) -> Option<&'a SymbolOccurrence> {
    let offset = offset_at_position(text, position)?;
    symbols
        .iter()
        .find(|symbol| symbol.span.start <= offset && offset <= symbol.span.end)
}

fn apply_change(text: &mut String, range: &Range, replacement: &str) {
    let Some(start) = offset_at_position(text, &range.start) else {
        return;
    };
    let Some(end) = offset_at_position(text, &range.end) else {
        return;
    };
    if start <= end {
        text.replace_range(start..end, replacement);
    }
}

/// LSP 的 character 使用 UTF-16 code unit，编译器 Span 使用 UTF-8 字节偏移。
fn offset_at_position(text: &str, position: &Position) -> Option<usize> {
    let mut start = 0;
    for line in 0..=position.line {
        let end = text[start..]
            .find('\n')
            .map(|index| start + index)
            .unwrap_or(text.len());
        if line == position.line {
            let mut utf16 = 0;
            for (index, ch) in text[start..end].char_indices() {
                if utf16 == position.character {
                    return Some(start + index);
                }
                utf16 += ch.len_utf16() as u64;
                if utf16 > position.character {
                    return None;
                }
            }
            return (utf16 == position.character).then_some(end);
        }
        if end == text.len() {
            return None;
        }
        start = end + 1;
    }
    None
}

fn span_to_range(text: &str, span: Span) -> Value {
    json!({ "start": position_at_offset(text, span.start), "end": position_at_offset(text, span.end) })
}

fn zero_range() -> Value {
    json!({ "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } })
}

fn position_at_offset(text: &str, offset: usize) -> Value {
    let offset = offset.min(text.len());
    let prefix = &text[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let character: usize = text[line_start..offset].chars().map(char::len_utf16).sum();
    json!({ "line": line, "character": character })
}

fn uri_to_path(uri: &str) -> PathBuf {
    let decoded = percent_decode(uri.strip_prefix("file://").unwrap_or(uri));
    #[cfg(windows)]
    let decoded = decoded
        .strip_prefix('/')
        .filter(|text| text.as_bytes().get(1) == Some(&b':'))
        .unwrap_or(&decoded)
        .to_string();
    PathBuf::from(decoded)
}

fn path_to_uri(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let text = raw
        .strip_prefix("\\\\?\\")
        .unwrap_or(&raw)
        .replace('\\', "/");
    #[cfg(windows)]
    let text = format!("/{text}");
    format!("file://{text}")
}

fn path_matches(left: &Path, right: &Path) -> bool {
    left == right || std::fs::canonicalize(left).ok() == std::fs::canonicalize(right).ok()
}

fn percent_decode(text: &str) -> String {
    let mut output = Vec::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(value) = u8::from_str_radix(&text[index + 1..index + 3], 16) {
                output.push(value);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn read_message(reader: &mut (impl BufRead + Read)) -> Option<String> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let trimmed = line
            .trim_end()
            .strip_prefix('\u{feff}')
            .unwrap_or(line.trim_end())
            .to_string();
        if trimmed.is_empty() {
            break;
        }
        if let Some(length) = trimmed.strip_prefix("Content-Length:") {
            content_length = length.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; content_length?];
    reader.read_exact(&mut body).ok()?;
    String::from_utf8(body).ok()
}

fn write_message(value: &Value) {
    let Ok(body) = serde_json::to_vec(value) else {
        return;
    };
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let _ = write!(output, "Content-Length: {}\r\n\r\n", body.len());
    let _ = output.write_all(&body);
    let _ = output.flush();
}

pub fn run_lsp() -> i32 {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut server = LspServer::default();
    while let Some(body) = read_message(&mut input) {
        let Ok(request) = serde_json::from_str::<Request>(&body) else {
            continue;
        };
        let exit = request.method == "exit";
        for message in server.handle(request) {
            write_message(&message);
        }
        if exit {
            break;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::{Position, Range, apply_change, offset_at_position};

    #[test]
    fn positions_use_utf16_and_incremental_changes() {
        let mut text = "const name = \"中\";\n".to_string();
        assert_eq!(
            offset_at_position(
                &text,
                &Position {
                    line: 0,
                    character: 15
                }
            ),
            Some(17)
        );
        apply_change(
            &mut text,
            &Range {
                start: Position {
                    line: 0,
                    character: 14,
                },
                end: Position {
                    line: 0,
                    character: 15,
                },
            },
            "文",
        );
        assert_eq!(text, "const name = \"文\";\n");
    }
}
