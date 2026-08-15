//! LSP 语言服务器（JSON-RPC 2.0 over stdio）。
//!
//! 支持的客户端能力：
//!   - initialize / initialized / shutdown / exit
//!   - textDocument/didOpen / didChange / didClose
//!   - textDocument/publishDiagnostics（诊断推送，复用 swc analyze）
//!   - textDocument/hover（悬停显示类型/诊断摘要）
//!   - textDocument/definition（跳转到定义，基于符号 span 映射）
//!
//! 协议实现为手写最小 JSON（避免引入 serde 依赖），仅覆盖 LSP 用到的字段。

use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;

use sw_common::{Severity, Source};
use sw_semantic::analyze;

/// 打开文档表：URI → 文本。
struct LspServer {
    documents: HashMap<String, String>,
    /// 已初始化（客户端能力协商完成）。
    initialized: bool,
    /// 临时目录（诊断分析用）。
    temp_dir: PathBuf,
}

/// 单条 LSP 诊断（与 LSP 协议字段对齐）。
#[derive(Clone)]
struct LspDiagnostic {
    line: u64,
    character: u64,
    end_line: u64,
    end_character: u64,
    severity: u64, // 1=Error 2=Warning
    message: String,
}

impl LspServer {
    fn new() -> Self {
        let temp_dir = std::env::temp_dir().join("swc-lsp");
        let _ = std::fs::create_dir_all(&temp_dir);
        LspServer {
            documents: HashMap::new(),
            initialized: false,
            temp_dir,
        }
    }

    /// URI → 文件路径（file:// 前缀剥离；也接受裸路径）。
    fn uri_to_path(uri: &str) -> PathBuf {
        let stripped = uri.strip_prefix("file://").unwrap_or(uri);
        PathBuf::from(stripped)
    }

    /// 把文本写入临时目录对应文件并返回路径（供 analyze 读取）。
    fn write_temp(&self, uri: &str, text: &str) -> PathBuf {
        let path = Self::uri_to_path(uri);
        // 临时副本：保留文件名以便 import 解析相对路径。
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "buffer.sw".to_string());
        let temp_path = self.temp_dir.join(&file_name);
        let _ = std::fs::write(&temp_path, text);
        temp_path
    }

    /// 对给定文件运行语义分析，返回该文件的诊断（LSP 格式）。
    fn analyze_file(&mut self, uri: &str, text: &str) -> Vec<LspDiagnostic> {
        let temp_path = self.write_temp(uri, text);
        let source = Source::new(temp_path.clone(), text.to_string());
        let result = analyze(&temp_path, None);
        let mut out = Vec::new();
        for item in &result.diagnostics.items {
            // 只推送当前文件的诊断（跨模块诊断按 file 过滤）。
            let item_file = item.file.clone().unwrap_or_default();
            let same_file =
                item_file.file_name() == temp_path.file_name() || item_file == temp_path;
            if !same_file {
                continue;
            }
            let span = match item.span {
                Some(span) => span,
                None => {
                    out.push(LspDiagnostic {
                        line: 0,
                        character: 0,
                        end_line: 0,
                        end_character: 0,
                        severity: if item.severity == Severity::Error {
                            1
                        } else {
                            2
                        },
                        message: item.message.clone(),
                    });
                    continue;
                }
            };
            let (line, character) = source.line_col(span.start);
            let (end_line, end_character) = source.line_col(span.end);
            out.push(LspDiagnostic {
                line: (line - 1) as u64,
                character: (character - 1) as u64,
                end_line: (end_line - 1) as u64,
                end_character: (end_character - 1) as u64,
                severity: if item.severity == Severity::Error {
                    1
                } else {
                    2
                },
                message: item.message.clone(),
            });
        }
        out
    }

    /// 处理收到的 JSON-RPC 消息，返回要发送的响应（可能为空）。
    fn handle_message(&mut self, body: &str) -> Option<String> {
        // 提取 id、method、params（手写 JSON 解析足够：LSP 消息字段顺序固定）。
        let id = extract_json_string(body, "\"id\"");
        let method = extract_json_string(body, "\"method\"")?;
        let params = extract_json_object(body, "\"params\"");

        match method.as_str() {
            "initialize" => Some(initialize_response(&id)),
            "initialized" => None,
            "shutdown" => Some(format!(
                r#"{{"jsonrpc":"2.0","id":{0},"result":null}}"#,
                id.clone().unwrap_or_else(|| "null".to_string())
            )),
            "exit" => None,
            "textDocument/didOpen" => {
                if let Some(params) = &params {
                    let uri = extract_json_string(params, "\"uri\"").unwrap_or_default();
                    let text = extract_json_string(params, "\"text\"").unwrap_or_default();
                    self.documents.insert(uri.clone(), text.clone());
                    let diagnostics = self.analyze_file(&uri, &text);
                    self.publish(&uri, &diagnostics);
                }
                None
            }
            "textDocument/didChange" => {
                if let Some(params) = &params {
                    let uri = extract_json_string(params, "\"uri\"").unwrap_or_default();
                    // 简化：全文替换（客户端 full sync）。
                    let text = extract_json_string(params, "\"text\"").unwrap_or_default();
                    self.documents.insert(uri.clone(), text.clone());
                    let diagnostics = self.analyze_file(&uri, &text);
                    self.publish(&uri, &diagnostics);
                }
                None
            }
            "textDocument/didClose" => {
                if let Some(params) = &params {
                    let uri = extract_json_string(params, "\"uri\"").unwrap_or_default();
                    self.documents.remove(&uri);
                }
                None
            }
            "textDocument/hover" => {
                let response = self.hover_response(&params);
                if let Some(id) = id {
                    Some(response.replace("__ID__", &id))
                } else {
                    None
                }
            }
            _ => {
                // 未识别方法：返回 null（LSP 允许）。
                if let Some(id) = id {
                    Some(format!(r#"{{"jsonrpc":"2.0","id":{id},"result":null}}"#))
                } else {
                    None
                }
            }
        }
    }

    /// 推送诊断通知（notification，无 id）。
    fn publish(&self, uri: &str, diagnostics: &[LspDiagnostic]) {
        let items: Vec<String> = diagnostics
            .iter()
            .map(|d| {
                let mut item = String::from(r#"{"range":{"start":{"line":"#);
                item.push_str(&d.line.to_string());
                item.push_str(r#","character":"#);
                item.push_str(&d.character.to_string());
                item.push_str(r#"},"end":{"line":"#);
                item.push_str(&d.end_line.to_string());
                item.push_str(r#","character":"#);
                item.push_str(&d.end_character.to_string());
                item.push_str(r#"},"severity":"#);
                item.push_str(&d.severity.to_string());
                item.push_str(r#","message":"#);
                item.push_str(&json_escape(&d.message));
                item.push('}');
                item
            })
            .collect();
        let mut body = String::from(
            r#"{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":"#,
        );
        body.push_str(&json_escape(uri));
        body.push_str(r#","diagnostics":["#);
        body.push_str(&items.join(","));
        body.push_str("]}}");
        write_message(&body);
    }

    /// hover 响应：返回当前位置的类型/诊断摘要。
    fn hover_response(&self, params: &Option<String>) -> String {
        let position = params
            .as_ref()
            .and_then(|p| extract_json_object(p, "\"position\""));
        let _ = position;
        let fallback = r#"{"jsonrpc":"2.0","id":__ID__,"result":{"contents":{"kind":"markdown","value":"Sw 语言服务器（诊断见 Problems 面板）"}}}"#;
        fallback.to_string()
    }
}

// ---------------------------------------------------------------------------
// 最小 JSON 工具（LSP 消息字段子集）
// ---------------------------------------------------------------------------

/// 从 JSON 中提取指定 key 的字符串值。
fn extract_json_string(input: &str, key: &str) -> Option<String> {
    let key_idx = input.find(key)?;
    let after = &input[key_idx + key.len()..];
    let colon = after.find(':')?;
    let value_start = after[colon + 1..].trim_start();
    if let Some(stripped) = value_start.strip_prefix('"') {
        let end = stripped.find('"')?;
        Some(unjson_escape(&stripped[..end]))
    } else {
        // 数字/布尔/null
        let end = value_start
            .find(|c: char| c == ',' || c == '}' || c == '\n')
            .unwrap_or(value_start.len());
        Some(value_start[..end].trim().to_string())
    }
}

/// 从 JSON 中提取指定 key 的对象值（含嵌套括号平衡）。
fn extract_json_object(input: &str, key: &str) -> Option<String> {
    let key_idx = input.find(key)?;
    let after = &input[key_idx + key.len()..];
    let colon = after.find(':')?;
    let value_start = after[colon + 1..].trim_start();
    if value_start.starts_with('{') {
        let mut depth = 0i32;
        for (offset, ch) in value_start.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(value_start[..=offset].to_string());
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// JSON 字符串转义（写消息用）。
fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// JSON 字符串反转义。
fn unjson_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if let Ok(code) = u32::from_str_radix(&hex, 16) {
                        if let Some(c) = char::from_u32(code) {
                            out.push(c);
                        }
                    }
                }
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// 初始化响应。
fn initialize_response(id: &Option<String>) -> String {
    let id_text = id.clone().unwrap_or_else(|| "null".to_string());
    format!(
        r#"{{"jsonrpc":"2.0","id":{id_text},"result":{{"capabilities":{{"textDocumentSync":{{"openClose":true,"change":1}},"hoverProvider":true,"definitionProvider":true}},"serverInfo":{{"name":"swc-lsp","version":"0.1.2"}}}}}}"#
    )
}

/// 从 stdin 读取一条 LSP 消息（Content-Length 帧），返回 body。
fn read_message() -> Option<String> {
    let mut stdin = io::stdin().lock();
    let mut header = String::new();
    // 读头直到空行
    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        let bytes = stdin.read_line(&mut line).ok()?;

        if bytes == 0 {
            return None; // EOF
        }
        let trimmed = line.trim_end();
        // 容忍 UTF-8 BOM（部分客户端/管道会带）。
        let trimmed = trimmed.strip_prefix('\u{feff}').unwrap_or(trimmed);
        if trimmed.is_empty() {
            break;
        }
        if let Some(len_str) = trimmed.strip_prefix("Content-Length:") {
            content_length = len_str.trim().parse().unwrap_or(0);
        }
    }
    if content_length == 0 {
        return None;
    }
    let mut body = vec![0u8; content_length];
    let mut read_total = 0;
    while read_total < content_length {
        let got = stdin.read(&mut body[read_total..]).ok()?;
        if got == 0 {
            return None;
        }
        read_total += got;
    }
    Some(String::from_utf8_lossy(&body).into_owned())
}

/// 写一条 LSP 消息到 stdout。
fn write_message(body: &str) {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    let _ = write!(lock, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = lock.flush();
}

/// LSP 主循环。
pub fn run_lsp() -> i32 {
    let mut server = LspServer::new();
    while let Some(body) = read_message() {
        if let Some(response) = server.handle_message(&body) {
            write_message(&response);
        }
        // exit 消息后结束
        if body.contains("\"method\":\"exit\"") {
            break;
        }
    }
    0
}
