#[derive(Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Word,
    String,
    LineComment,
    BlockComment,
    OpenBrace,
    CloseBrace,
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    Comma,
    Semicolon,
    Colon,
    Dot,
    Operator,
    Other,
}

struct Token<'a> {
    text: &'a str,
    kind: TokenKind,
}

/// 保守格式化：字面量和注释原样保留，只规范词法单元之间的空白、缩进与换行。
pub fn format_source(source: &str) -> String {
    let tokens = tokenize(source);
    let mut output = String::new();
    let mut indent = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut braces = Vec::new();
    let mut previous: Option<TokenKind> = None;
    let mut previous_text = "";
    let mut line_start = true;

    for (index, token) in tokens.iter().enumerate() {
        let next = tokens.get(index + 1).map(|item| item.kind);
        match token.kind {
            TokenKind::LineComment => {
                if !line_start {
                    space(&mut output);
                }
                write_indent(&mut output, indent, &mut line_start);
                output.push_str(token.text);
                newline(&mut output, &mut line_start);
            }
            TokenKind::BlockComment => {
                if needs_space(previous, token.kind, previous_text) {
                    space(&mut output);
                }
                write_indent(&mut output, indent, &mut line_start);
                output.push_str(token.text);
                if token.text.contains('\n') {
                    newline(&mut output, &mut line_start);
                }
            }
            TokenKind::OpenBrace => {
                if needs_space(previous, token.kind, previous_text) {
                    space(&mut output);
                }
                write_indent(&mut output, indent, &mut line_start);
                output.push('{');
                braces.push(paren_depth == 0 && bracket_depth == 0);
                indent += 1;
                newline(&mut output, &mut line_start);
            }
            TokenKind::CloseBrace => {
                indent = indent.saturating_sub(1);
                if !line_start {
                    newline(&mut output, &mut line_start);
                }
                write_indent(&mut output, indent, &mut line_start);
                output.push('}');
                let top_level = braces.pop().unwrap_or(false);
                if top_level
                    && matches!(
                        next,
                        Some(TokenKind::Word | TokenKind::LineComment | TokenKind::BlockComment)
                    )
                {
                    newline(&mut output, &mut line_start);
                }
            }
            TokenKind::Semicolon => {
                write_indent(&mut output, indent, &mut line_start);
                output.push(';');
                if paren_depth == 0 {
                    newline(&mut output, &mut line_start);
                } else {
                    space(&mut output);
                }
            }
            TokenKind::Comma => {
                output.push(',');
                if braces.last().copied().unwrap_or(false) && paren_depth == 0 && bracket_depth == 0
                {
                    newline(&mut output, &mut line_start);
                } else {
                    space(&mut output);
                }
            }
            TokenKind::Colon => {
                output.push(':');
                space(&mut output);
            }
            TokenKind::OpenParen => {
                if matches!(previous, Some(TokenKind::Word))
                    && matches!(
                        previous_text,
                        "if" | "for" | "while" | "switch" | "match" | "catch"
                    )
                {
                    space(&mut output);
                }
                output.push('(');
                paren_depth += 1;
            }
            TokenKind::CloseParen => {
                output.push(')');
                paren_depth = paren_depth.saturating_sub(1);
            }
            TokenKind::OpenBracket => {
                output.push('[');
                bracket_depth += 1;
            }
            TokenKind::CloseBracket => {
                output.push(']');
                bracket_depth = bracket_depth.saturating_sub(1);
            }
            TokenKind::Dot => output.push_str(token.text),
            TokenKind::Operator => {
                if token.text == "!"
                    || token.text == "~"
                    || token.text == "++"
                    || token.text == "--"
                    || token.text == "?"
                {
                    output.push_str(token.text);
                } else {
                    space(&mut output);
                    output.push_str(token.text);
                    space(&mut output);
                }
            }
            _ => {
                if needs_space(previous, token.kind, previous_text) {
                    space(&mut output);
                }
                write_indent(&mut output, indent, &mut line_start);
                output.push_str(token.text);
            }
        }
        previous = Some(token.kind);
        previous_text = token.text;
    }
    while output.ends_with([' ', '\n']) {
        output.pop();
    }
    output.push('\n');
    output
}

fn needs_space(previous: Option<TokenKind>, current: TokenKind, previous_text: &str) -> bool {
    matches!(
        previous,
        Some(
            TokenKind::Word
                | TokenKind::String
                | TokenKind::CloseParen
                | TokenKind::CloseBracket
                | TokenKind::CloseBrace
        )
    ) && matches!(
        current,
        TokenKind::Word | TokenKind::String | TokenKind::OpenBrace
    ) && previous_text != ""
}

fn write_indent(output: &mut String, indent: usize, line_start: &mut bool) {
    if *line_start {
        output.push_str(&"    ".repeat(indent));
        *line_start = false;
    }
}

fn space(output: &mut String) {
    if !output.is_empty() && !output.ends_with([' ', '\n']) {
        output.push(' ');
    }
}

fn newline(output: &mut String, line_start: &mut bool) {
    while output.ends_with(' ') {
        output.pop();
    }
    if !output.ends_with('\n') {
        output.push('\n');
    }
    *line_start = true;
}

fn tokenize(source: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < source.len() {
        let rest = &source[index..];
        let ch = rest.chars().next().unwrap();
        if ch.is_whitespace() {
            index += ch.len_utf8();
            continue;
        }
        if rest.starts_with("//") {
            let end = rest
                .find('\n')
                .map(|offset| index + offset)
                .unwrap_or(source.len());
            tokens.push(Token {
                text: &source[index..end],
                kind: TokenKind::LineComment,
            });
            index = end;
            continue;
        }
        if rest.starts_with("/*") {
            let end = rest
                .find("*/")
                .map(|offset| index + offset + 2)
                .unwrap_or(source.len());
            tokens.push(Token {
                text: &source[index..end],
                kind: TokenKind::BlockComment,
            });
            index = end;
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            let quote = ch;
            let mut end = index + quote.len_utf8();
            let mut escaped = false;
            while end < source.len() {
                let current = source[end..].chars().next().unwrap();
                end += current.len_utf8();
                if escaped {
                    escaped = false;
                    continue;
                }
                if current == '\\' {
                    escaped = true;
                    continue;
                }
                if current == quote {
                    break;
                }
            }
            tokens.push(Token {
                text: &source[index..end],
                kind: TokenKind::String,
            });
            index = end;
            continue;
        }
        if ch == '_' || ch == '$' || ch.is_alphanumeric() {
            let mut end = index + ch.len_utf8();
            while end < source.len() {
                let current = source[end..].chars().next().unwrap();
                if current == '_' || current == '$' || current.is_alphanumeric() {
                    end += current.len_utf8();
                } else {
                    break;
                }
            }
            tokens.push(Token {
                text: &source[index..end],
                kind: TokenKind::Word,
            });
            index = end;
            continue;
        }
        let (length, kind) = punctuation(rest);
        tokens.push(Token {
            text: &source[index..index + length],
            kind,
        });
        index += length;
    }
    tokens
}

fn punctuation(rest: &str) -> (usize, TokenKind) {
    for operator in [
        "...", "<<=", ">>=", "&&=", "||=", "??=", "**", "++", "--", "+=", "-=", "*=", "/=", "%=",
        "&=", "|=", "^=", "==", "!=", "<=", ">=", "&&", "||", "<<", ">>", "??", "?.", "=>",
    ] {
        if rest.starts_with(operator) {
            return (
                operator.len(),
                if operator == "?." {
                    TokenKind::Dot
                } else {
                    TokenKind::Operator
                },
            );
        }
    }
    let ch = rest.chars().next().unwrap();
    let kind = match ch {
        '{' => TokenKind::OpenBrace,
        '}' => TokenKind::CloseBrace,
        '(' => TokenKind::OpenParen,
        ')' => TokenKind::CloseParen,
        '[' => TokenKind::OpenBracket,
        ']' => TokenKind::CloseBracket,
        ',' => TokenKind::Comma,
        ';' => TokenKind::Semicolon,
        ':' => TokenKind::Colon,
        '.' => TokenKind::Dot,
        '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>' | '&' | '|' | '^' | '!' | '~' | '?' => {
            TokenKind::Operator
        }
        _ => TokenKind::Other,
    };
    (ch.len_utf8(), kind)
}

#[cfg(test)]
mod tests {
    use super::format_source;

    #[test]
    fn formats_code_and_preserves_comments() {
        assert_eq!(
            format_source("function main():int{let x=1;// hi\nreturn x;}"),
            "function main(): int {\n    let x = 1;\n    // hi\n    return x;\n}\n"
        );
    }
}
