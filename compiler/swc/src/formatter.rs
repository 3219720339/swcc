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
    let mut prev_prev_text = "";
    let mut line_start = true;
    // 未配对的三元 `?` 计数：遇到三元中缀 `?` 加一，遇到配对 `:` 减一并
    // 让 `:` 前后留空格；否则 `:` 按对象字段/类型注解处理（前无空格）。
    let mut ternary_depth = 0usize;
    // 泛型尖括号嵌套深度：`Box<int>` 内为 1，`Box<Box<int>>` 内为 2。
    // `generic_depth > 0` 时 `<`/`>` 紧贴（泛型），否则按比较运算符留空格。
    let mut generic_depth = 0usize;

    for (index, token) in tokens.iter().enumerate() {
        let next = tokens.get(index + 1).map(|item| item.kind);
        let next_text = tokens.get(index + 1).map(|item| item.text);
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
                // `{` 前留空格：普通标识符/`)`/`]`/泛型闭合 `>` 之后。
                if needs_space(previous, token.kind, previous_text)
                    || (previous == Some(TokenKind::Operator) && previous_text == ">")
                {
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
                if generic_depth == 0
                    && braces.last().copied().unwrap_or(false)
                    && paren_depth == 0
                    && bracket_depth == 0
                {
                    newline(&mut output, &mut line_start);
                } else {
                    space(&mut output);
                }
            }
            TokenKind::Colon => {
                if ternary_depth > 0 {
                    // 三元 `a ? b : c` 的冒号：前后留空格。
                    space(&mut output);
                    output.push(':');
                    space(&mut output);
                    ternary_depth -= 1;
                } else {
                    // 对象字段 / 类型注解 `x: int`：前无空格、后有空格。
                    output.push(':');
                    space(&mut output);
                }
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
                {
                    output.push_str(token.text);
                } else if token.text == "?" {
                    // `?` 三种语义按下一个 token 区分：
                    //  1) 三元中缀 `a ? b : c`：`?` 后是表达式开头 → 前后留空格；
                    //  2) 可空类型后缀 `int?`：`?` 后是 `{`（函数体）或边界 →
                    //     前无空格，`{` 前需空格；
                    //  3) TryOp 后缀 `expr?`：`?` 后是 `;`/`)` 等边界 → 前无空格。
                    let ternary = matches!(
                        next,
                        Some(
                            TokenKind::Word
                                | TokenKind::String
                                | TokenKind::OpenParen
                                | TokenKind::OpenBracket
                        )
                    ) || matches!(next_text, Some("!" | "~" | "-" | "+"));
                    if ternary {
                        space(&mut output);
                        output.push('?');
                        space(&mut output);
                        ternary_depth += 1;
                    } else {
                        output.push('?');
                        if next == Some(TokenKind::OpenBrace) {
                            space(&mut output);
                        }
                    }
                } else if token.text == "-" || token.text == "+" {
                    // 一元 `-`/`+`（前一个 token 是运算符/括号/逗号/冒号等）
                    // 后紧跟操作数；二元则前后留空格。`(`/`[`/`{` 后的一元
                    // 不加前空格（`f(-1)`、`[-1]`）。
                    let unary = previous
                        .map(|kind| {
                            matches!(
                                kind,
                                TokenKind::Operator
                                    | TokenKind::OpenParen
                                    | TokenKind::OpenBracket
                                    | TokenKind::Comma
                                    | TokenKind::Colon
                                    | TokenKind::OpenBrace
                            )
                        })
                        .unwrap_or(true);
                    if unary {
                        if !matches!(
                            previous,
                            Some(
                                TokenKind::OpenParen
                                    | TokenKind::OpenBracket
                                    | TokenKind::OpenBrace
                            )
                        ) {
                            space(&mut output);
                        }
                        output.push_str(token.text);
                    } else {
                        space(&mut output);
                        output.push_str(token.text);
                        space(&mut output);
                    }
                } else if token.text == "<"
                    || token.text == ">"
                    || token.text == ">>"
                    || token.text == ">>="
                {
                    // 泛型尖括号 vs 比较运算符：`Box<int>`、`Result<T, E>` 的
                    // `<`/`>` 紧贴；`1 < 2`、`x > 0` 的比较前后留空格。
                    // 判定 `<` 为泛型开：前一个是名字且再前一个是类型上下文
                    // （function/class/struct/enum/interface 声明名、
                    //  implements/extends/where/new 之后、`: ` 类型注解），
                    // 或在泛型嵌套内（generic_depth > 0）。
                    let opens_generic = if token.text == "<" {
                        generic_depth > 0
                            || (previous == Some(TokenKind::Word)
                                && matches!(
                                    prev_prev_text,
                                    "function"
                                        | "class"
                                        | "struct"
                                        | "enum"
                                        | "interface"
                                        | "implements"
                                        | "extends"
                                        | "where"
                                        | "new"
                                        | ":"
                                ))
                    } else {
                        false
                    };
                    if opens_generic {
                        output.push('<');
                        generic_depth += 1;
                    } else if token.text == ">" && generic_depth > 0 {
                        output.push('>');
                        generic_depth -= 1;
                    } else if token.text == ">>" && generic_depth >= 2 {
                        // 嵌套泛型闭合 `Box<Box<int>>`：`>>` 拆成两个 `>`。
                        output.push_str(">>");
                        generic_depth -= 2;
                    } else if token.text == ">>=" && generic_depth >= 2 {
                        // 泛型闭合后跟赋值 `Box<Box<int>>=x`：`>>=` 拆成
                        // `>>`（两个闭合 `>`）+ `=`。
                        output.push_str(">>");
                        generic_depth -= 2;
                        space(&mut output);
                        output.push('=');
                        space(&mut output);
                    } else {
                        space(&mut output);
                        output.push_str(token.text);
                        space(&mut output);
                    }
                } else {
                    space(&mut output);
                    output.push_str(token.text);
                    space(&mut output);
                }
            }
            _ => {
                // 泛型闭合 `>` 后接标识符（如 `class Box<T>implements`）需空格。
                let after_generic = previous == Some(TokenKind::Operator)
                    && matches!(previous_text, ">" | ">>")
                    && matches!(
                        token.kind,
                        TokenKind::Word | TokenKind::String | TokenKind::OpenBrace
                    );
                if needs_space(previous, token.kind, previous_text) || after_generic {
                    space(&mut output);
                }
                write_indent(&mut output, indent, &mut line_start);
                output.push_str(token.text);
            }
        }
        prev_prev_text = previous_text;
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

    #[test]
    fn formats_ternary_with_spaces_around_q_and_colon() {
        assert_eq!(
            format_source("const x=a>=0.0?0:1;"),
            "const x = a >= 0.0 ? 0 : 1;\n"
        );
    }

    #[test]
    fn keeps_nullable_type_and_tryop_suffix_tight() {
        assert_eq!(
            format_source("function parse(x:int):int?{return x>0?x:null;}\nconst v=parse(5)?;"),
            "function parse(x: int): int? {\n    return x > 0 ? x : null;\n}\nconst v = parse(5)?;\n"
        );
    }

    #[test]
    fn keeps_unary_minus_tight_in_ternary_and_args() {
        assert_eq!(
            format_source("const y=b>0?-1:2;\nconst f=fn_call(-1,+2);"),
            "const y = b > 0 ? -1 : 2;\nconst f = fn_call(-1, +2);\n"
        );
    }

    #[test]
    fn formats_binary_minus_with_spaces() {
        assert_eq!(
            format_source("const sub=a-b;\nconst neg=-a;"),
            "const sub = a - b;\nconst neg = -a;\n"
        );
    }

    #[test]
    fn keeps_generic_angle_brackets_tight() {
        assert_eq!(
            format_source(
                "enum Result<T, E>{Ok(T),Err(E)}\nfunction make<T>(x:T):Box<T>{return new Box<T>(x);}"
            ),
            "enum Result<T, E> {\n    Ok(T),\n    Err(E)\n}\nfunction make<T>(x: T): Box<T> {\n    return new Box<T>(x);\n}\n"
        );
    }

    #[test]
    fn keeps_nested_generic_angle_brackets_tight() {
        assert_eq!(
            format_source("const d:Box<Box<int>>=new Box<Box<int>>(null);"),
            "const d: Box<Box<int>> = new Box<Box<int>>(null);\n"
        );
    }

    #[test]
    fn keeps_comparison_operators_with_spaces() {
        assert_eq!(
            format_source("const a=1<2;\nconst b=x>0?1:2;"),
            "const a = 1 < 2;\nconst b = x > 0 ? 1 : 2;\n"
        );
    }

    #[test]
    fn keeps_generic_class_implements_with_space() {
        assert_eq!(
            format_source("class Box<T>implements Container<T>{value:T;}"),
            "class Box<T> implements Container<T> {\n    value: T;\n}\n"
        );
    }
}
