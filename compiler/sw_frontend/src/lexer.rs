use std::collections::VecDeque;

use sw_common::{Diagnostics, Source, Span};

use crate::token::{FloatSuffix, IntegerSuffix, Keyword, Token, TokenKind};

/// 词法分析器：按需产出 token，支持解析器回退（checkpoint/restore）。
pub struct Lexer<'a> {
    source: &'a Source,
    pos: usize,
    template_text_mode: bool,
    buffer: VecDeque<Token>,
    diagnostics: &'a mut Diagnostics,
}

#[derive(Clone, Copy, Debug)]
pub struct Checkpoint {
    pos: usize,
    template_text_mode: bool,
    buffer_len: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a Source, diagnostics: &'a mut Diagnostics) -> Self {
        Self {
            source,
            pos: 0,
            template_text_mode: false,
            buffer: VecDeque::new(),
            diagnostics,
        }
    }

    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            pos: self.pos,
            template_text_mode: self.template_text_mode,
            buffer_len: self.buffer.len(),
        }
    }

    pub fn restore(&mut self, checkpoint: Checkpoint) {
        self.pos = checkpoint.pos;
        self.template_text_mode = checkpoint.template_text_mode;
        self.buffer.truncate(checkpoint.buffer_len);
    }

    /// 表达式结束后继续扫描模板文本：调用方必须已经消费了 `}`。
    pub fn resume_template(&mut self) {
        self.template_text_mode = true;
    }

    pub fn next_token(&mut self) -> Token {
        if let Some(token) = self.buffer.pop_front() {
            return token;
        }
        if self.template_text_mode {
            self.lex_template_text()
        } else {
            self.lex_normal()
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.source.text[self.pos..].chars().next()
    }

    fn peek_char_at(&self, offset: usize) -> Option<char> {
        self.source.text[self.pos + offset..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.peek_char()?;
        self.pos += character.len_utf8();
        Some(character)
    }

    pub fn error(&mut self, message: impl Into<String>, span: Span) {
        self.diagnostics.error(message, Some(span));
    }

    pub fn diagnostics_len(&self) -> usize {
        self.diagnostics.items.len()
    }

    pub fn truncate_diagnostics(&mut self, len: usize) {
        self.diagnostics.items.truncate(len);
    }

    fn lex_normal(&mut self) -> Token {
        self.skip_trivia();
        let start = self.pos;
        let Some(character) = self.peek_char() else {
            return Token::eof(self.pos);
        };

        if is_ident_start(character) {
            return self.lex_identifier(start);
        }
        if character.is_ascii_digit() {
            return self.lex_number(start);
        }

        let token = match character {
            '"' | '\'' => return self.lex_string(start, character),
            '`' => {
                self.bump();
                self.template_text_mode = true;
                Token {
                    kind: TokenKind::TemplateStart,
                    span: Span::new(start, self.pos),
                }
            }
            '(' => self.single(TokenKind::LParen, start),
            ')' => self.single(TokenKind::RParen, start),
            '{' => self.single(TokenKind::LBrace, start),
            '}' => self.single(TokenKind::RBrace, start),
            '[' => self.single(TokenKind::LBracket, start),
            ']' => self.single(TokenKind::RBracket, start),
            ',' => self.single(TokenKind::Comma, start),
            ';' => self.single(TokenKind::Semicolon, start),
            ':' => self.single(TokenKind::Colon, start),
            '@' => self.single(TokenKind::At, start),
            '+' => {
                if self.at("++") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::PlusPlus,
                        span: Span::new(start, self.pos),
                    }
                } else if self.at("+=") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::PlusAssign,
                        span: Span::new(start, self.pos),
                    }
                } else {
                    self.single(TokenKind::Plus, start)
                }
            }
            '-' => {
                if self.at("--") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::MinusMinus,
                        span: Span::new(start, self.pos),
                    }
                } else if self.at("-=") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::MinusAssign,
                        span: Span::new(start, self.pos),
                    }
                } else {
                    self.single(TokenKind::Minus, start)
                }
            }
            '*' => {
                if self.at("**") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::StarStar,
                        span: Span::new(start, self.pos),
                    }
                } else if self.at("*=") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::StarAssign,
                        span: Span::new(start, self.pos),
                    }
                } else {
                    self.single(TokenKind::Star, start)
                }
            }
            '/' => {
                if self.at("/=") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::SlashAssign,
                        span: Span::new(start, self.pos),
                    }
                } else {
                    self.single(TokenKind::Slash, start)
                }
            }
            '%' => {
                if self.at("%=") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::PercentAssign,
                        span: Span::new(start, self.pos),
                    }
                } else {
                    self.single(TokenKind::Percent, start)
                }
            }
            '=' => {
                if self.at("==") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::Eq,
                        span: Span::new(start, self.pos),
                    }
                } else if self.at("=>") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::FatArrow,
                        span: Span::new(start, self.pos),
                    }
                } else {
                    self.single(TokenKind::Assign, start)
                }
            }
            '!' => {
                if self.at("!=") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::Ne,
                        span: Span::new(start, self.pos),
                    }
                } else {
                    self.single(TokenKind::Bang, start)
                }
            }
            '<' => {
                if self.at("<<=") {
                    self.bump_thrice();
                    Token {
                        kind: TokenKind::ShlAssign,
                        span: Span::new(start, self.pos),
                    }
                } else if self.at("<<") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::Shl,
                        span: Span::new(start, self.pos),
                    }
                } else if self.at("<=") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::Le,
                        span: Span::new(start, self.pos),
                    }
                } else {
                    self.single(TokenKind::Lt, start)
                }
            }
            '>' => {
                if self.at(">>=") {
                    self.bump_thrice();
                    Token {
                        kind: TokenKind::ShrAssign,
                        span: Span::new(start, self.pos),
                    }
                } else if self.at(">>") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::Shr,
                        span: Span::new(start, self.pos),
                    }
                } else if self.at(">=") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::Ge,
                        span: Span::new(start, self.pos),
                    }
                } else {
                    self.single(TokenKind::Gt, start)
                }
            }
            '&' => {
                if self.at("&&") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::AmpAmp,
                        span: Span::new(start, self.pos),
                    }
                } else if self.at("&=") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::AmpAssign,
                        span: Span::new(start, self.pos),
                    }
                } else {
                    self.single(TokenKind::Amp, start)
                }
            }
            '|' => {
                if self.at("||") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::PipePipe,
                        span: Span::new(start, self.pos),
                    }
                } else if self.at("|=") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::PipeAssign,
                        span: Span::new(start, self.pos),
                    }
                } else {
                    self.single(TokenKind::Pipe, start)
                }
            }
            '^' => {
                if self.at("^=") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::CaretAssign,
                        span: Span::new(start, self.pos),
                    }
                } else {
                    self.single(TokenKind::Caret, start)
                }
            }
            '?' => {
                if self.at("??=") {
                    self.bump_thrice();
                    Token {
                        kind: TokenKind::CoalesceAssign,
                        span: Span::new(start, self.pos),
                    }
                } else if self.at("??") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::QuestionQuestion,
                        span: Span::new(start, self.pos),
                    }
                } else if self.at("?.") {
                    self.bump_twice();
                    Token {
                        kind: TokenKind::QuestionDot,
                        span: Span::new(start, self.pos),
                    }
                } else {
                    self.single(TokenKind::Question, start)
                }
            }
            '~' => self.single(TokenKind::Tilde, start),
            '.' => {
                if self.at("...") {
                    self.bump_thrice();
                    Token {
                        kind: TokenKind::DotDotDot,
                        span: Span::new(start, self.pos),
                    }
                } else {
                    self.single(TokenKind::Dot, start)
                }
            }
            _ => {
                self.bump();
                self.error(
                    format!("无法识别的字符 `{character}`"),
                    Span::new(start, self.pos),
                );
                return self.lex_normal();
            }
        };
        token
    }

    fn single(&mut self, kind: TokenKind, start: usize) -> Token {
        self.bump();
        Token {
            kind,
            span: Span::new(start, self.pos),
        }
    }

    fn bump_twice(&mut self) {
        self.bump();
        self.bump();
    }

    fn bump_thrice(&mut self) {
        self.bump();
        self.bump();
        self.bump();
    }

    fn at(&self, text: &str) -> bool {
        self.source.text[self.pos..].starts_with(text)
    }

    fn skip_trivia(&mut self) {
        loop {
            match self.peek_char() {
                Some(character) if character.is_whitespace() => {
                    self.bump();
                }
                Some('/') if self.at("//") => {
                    self.bump_twice();
                    while let Some(character) = self.peek_char() {
                        if character == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                Some('/') if self.at("/*") => {
                    let start = self.pos;
                    self.bump_twice();
                    let mut depth = 1usize;
                    while depth > 0 {
                        match self.peek_char() {
                            None => {
                                self.error("块注释未闭合", Span::new(start, self.pos));
                                return;
                            }
                            Some('/') if self.at("/*") => {
                                self.bump_twice();
                                depth += 1;
                            }
                            Some('*') if self.at("*/") => {
                                self.bump_twice();
                                depth -= 1;
                            }
                            _ => {
                                self.bump();
                            }
                        }
                    }
                }
                _ => return,
            }
        }
    }

    fn lex_identifier(&mut self, start: usize) -> Token {
        while let Some(character) = self.peek_char() {
            if is_ident_continue(character) {
                self.bump();
            } else {
                break;
            }
        }
        let text = &self.source.text[start..self.pos];
        let kind = match Keyword::from_str(text) {
            Some(keyword) => TokenKind::Keyword(keyword),
            None => TokenKind::Ident(text.to_owned()),
        };
        Token {
            kind,
            span: Span::new(start, self.pos),
        }
    }

    fn lex_number(&mut self, start: usize) -> Token {
        let mut base = 10;
        if self.peek_char() == Some('0') {
            match self.peek_char_at(1) {
                Some('x') | Some('X') => {
                    base = 16;
                    self.bump_twice();
                }
                Some('b') | Some('B') => {
                    base = 2;
                    self.bump_twice();
                }
                Some('o') | Some('O') => {
                    base = 8;
                    self.bump_twice();
                }
                _ => {}
            }
        }

        let digits_start = self.pos;
        self.scan_digits(base);
        if self.pos == digits_start {
            self.error(
                format!("数字前缀缺少数字（base {base}）"),
                Span::new(start, self.pos),
            );
            return self.recover_invalid_token(start);
        }

        let mut is_float = false;
        if base == 10 && self.peek_char() == Some('.') {
            if let Some(next) = self.peek_char_at(1) {
                if next.is_ascii_digit() {
                    self.bump();
                    self.scan_digits(10);
                    is_float = true;
                }
            }
        }

        if base == 10 && matches!(self.peek_char(), Some('e') | Some('E')) {
            let after = self.peek_char_at(1);
            let exponent_start = match after {
                Some('+') | Some('-') => {
                    matches!(self.peek_char_at(2), Some(d) if d.is_ascii_digit())
                }
                Some(d) => d.is_ascii_digit(),
                None => false,
            };
            if exponent_start {
                self.bump();
                if matches!(self.peek_char(), Some('+') | Some('-')) {
                    self.bump();
                }
                self.scan_digits(10);
                is_float = true;
            }
        }

        let text_end = self.pos;
        let (suffix, suffix_span) = self.scan_suffix();
        let number_end = self.pos;
        if self.peek_char().is_some_and(is_ident_continue) {
            let next = self.peek_char().unwrap();
            let message = if suffix.is_some() {
                format!("数字后缀后不能紧跟标识符字符 `{next}`")
            } else {
                format!("数字后不能紧跟标识符字符 `{next}`")
            };
            self.error(message, Span::new(number_end, number_end + next.len_utf8()));
        }

        let text = self.source.text[start..text_end].to_owned();
        let kind = match suffix {
            Some(Suffix::Integer(inner)) => {
                if is_float {
                    self.error("浮点数字面量不能使用整数后缀", suffix_span);
                }
                TokenKind::Integer {
                    text,
                    suffix: Some(inner),
                }
            }
            Some(Suffix::Float(inner)) => TokenKind::Float {
                text,
                suffix: Some(inner),
            },
            None if is_float => TokenKind::Float { text, suffix: None },
            None => TokenKind::Integer { text, suffix: None },
        };
        Token {
            kind,
            span: Span::new(start, number_end),
        }
    }

    fn scan_digits(&mut self, base: u32) {
        let start = self.pos;
        while let Some(character) = self.peek_char() {
            if character == '_' || is_base_digit(character, base) {
                self.bump();
            } else {
                break;
            }
        }
        let scanned = &self.source.text[start..self.pos];
        let characters: Vec<char> = scanned.chars().collect();
        for (index, character) in characters.iter().enumerate() {
            if *character != '_' {
                continue;
            }
            let prev_ok = index > 0 && is_base_digit(characters[index - 1], base);
            let next_ok =
                index + 1 < characters.len() && is_base_digit(characters[index + 1], base);
            if prev_ok && next_ok {
                continue;
            }
            let byte_offset = scanned
                .char_indices()
                .nth(index)
                .map(|(offset, _)| start + offset)
                .unwrap_or(self.pos);
            self.error(
                "数字分隔符 `_` 只能出现在数字之间",
                Span::new(byte_offset, byte_offset + 1),
            );
        }
    }

    fn scan_suffix(&mut self) -> (Option<Suffix>, Span) {
        let start = self.pos;
        let mut text = String::new();
        while let Some(character) = self.peek_char() {
            if character.is_ascii_alphanumeric() {
                text.push(character);
                self.bump();
            } else {
                break;
            }
        }
        let span = Span::new(start, self.pos);
        let suffix = match text.as_str() {
            "i8" => Some(Suffix::Integer(IntegerSuffix::I8)),
            "i16" => Some(Suffix::Integer(IntegerSuffix::I16)),
            "i32" => Some(Suffix::Integer(IntegerSuffix::I32)),
            "i64" => Some(Suffix::Integer(IntegerSuffix::I64)),
            "isize" => Some(Suffix::Integer(IntegerSuffix::Isize)),
            "u8" => Some(Suffix::Integer(IntegerSuffix::U8)),
            "u16" => Some(Suffix::Integer(IntegerSuffix::U16)),
            "u32" => Some(Suffix::Integer(IntegerSuffix::U32)),
            "u64" => Some(Suffix::Integer(IntegerSuffix::U64)),
            "usize" => Some(Suffix::Integer(IntegerSuffix::Usize)),
            "f32" => Some(Suffix::Float(FloatSuffix::F32)),
            "f64" => Some(Suffix::Float(FloatSuffix::F64)),
            _ => {
                self.pos = start;
                None
            }
        };
        (suffix, span)
    }

    /// 数字出错后的恢复：把已消费的字符当作一个整数 token，避免无限循环。
    fn recover_invalid_token(&mut self, start: usize) -> Token {
        while let Some(character) = self.peek_char() {
            if character.is_ascii_alphanumeric() || character == '_' {
                self.bump();
            } else {
                break;
            }
        }
        Token {
            kind: TokenKind::Integer {
                text: self.source.text[start..self.pos].to_owned(),
                suffix: None,
            },
            span: Span::new(start, self.pos),
        }
    }

    fn lex_string(&mut self, start: usize, quote: char) -> Token {
        self.bump(); // 开引号
        let mut value = String::new();
        let mut raw_bytes = Vec::new();

        loop {
            let Some(character) = self.peek_char() else {
                self.error("字符串未闭合", Span::new(start, self.pos));
                break;
            };
            if character == quote {
                self.bump();
                break;
            }
            if character == '\n' {
                self.error("字符串不能包含未转义的换行", Span::new(start, self.pos));
                break;
            }
            if character == '\\' {
                let escape_start = self.pos;
                self.bump();
                match self.parse_escape() {
                    Ok(EscapeValue::Char(escaped)) => {
                        if self.flush_raw_bytes(&mut raw_bytes, &mut value, start) {
                            return self.finish_string(start, quote, value);
                        }
                        value.push(escaped);
                    }
                    Ok(EscapeValue::Bytes(byte)) => {
                        raw_bytes.push(byte);
                    }
                    Err(message) => {
                        self.error(message, Span::new(escape_start, self.pos));
                    }
                }
            } else {
                if self.flush_raw_bytes(&mut raw_bytes, &mut value, start) {
                    return self.finish_string(start, quote, value);
                }
                value.push(character);
                self.bump();
            }
        }
        let _ = self.flush_raw_bytes(&mut raw_bytes, &mut value, start);
        self.finish_string(start, quote, value)
    }

    /// 把连续 \x 转义字节刷入字符串；失败时报告错误并返回 true 表示终止。
    fn flush_raw_bytes(
        &mut self,
        raw_bytes: &mut Vec<u8>,
        value: &mut String,
        start: usize,
    ) -> bool {
        if raw_bytes.is_empty() {
            return false;
        }
        let bytes = std::mem::take(raw_bytes);
        match String::from_utf8(bytes) {
            Ok(text) => {
                value.push_str(&text);
                false
            }
            Err(_) => {
                self.error(
                    "\\x 转义产生的字节不是合法 UTF-8",
                    Span::new(start, self.pos),
                );
                true
            }
        }
    }

    fn finish_string(&mut self, start: usize, quote: char, value: String) -> Token {
        let end = self.pos;
        let character_count = value.chars().count();
        let kind = if quote == '\'' && character_count == 1 {
            TokenKind::Char(value.chars().next().unwrap())
        } else {
            TokenKind::Str(value)
        };
        Token {
            kind,
            span: Span::new(start, end),
        }
    }

    fn parse_escape(&mut self) -> Result<EscapeValue, String> {
        let Some(character) = self.bump() else {
            return Err("转义序列缺少字符".to_owned());
        };
        match character {
            '0' => Ok(EscapeValue::Char('\0')),
            't' => Ok(EscapeValue::Char('\t')),
            'n' => Ok(EscapeValue::Char('\n')),
            'r' => Ok(EscapeValue::Char('\r')),
            '"' => Ok(EscapeValue::Char('"')),
            '\'' => Ok(EscapeValue::Char('\'')),
            '`' => Ok(EscapeValue::Char('`')),
            '\\' => Ok(EscapeValue::Char('\\')),
            'x' => {
                let high = self.hex_digit().ok_or("\\x 后需要两位十六进制数字")?;
                let low = self.hex_digit().ok_or("\\x 后需要两位十六进制数字")?;
                Ok(EscapeValue::Bytes((high << 4) | low))
            }
            'u' => {
                if self.bump() != Some('{') {
                    return Err("\\u 后需要 `{`".to_owned());
                }
                let mut value = 0u32;
                let mut count = 0usize;
                while let Some(character) = self.peek_char() {
                    if character == '}' {
                        self.bump();
                        break;
                    }
                    let digit = character
                        .to_digit(16)
                        .ok_or("\\u{...} 中存在非法十六进制数字")?;
                    value = value * 16 + digit;
                    count += 1;
                    if count > 6 {
                        return Err("\\u{...} 超过 6 位十六进制数字".to_owned());
                    }
                    self.bump();
                }
                if count == 0 {
                    return Err("\\u{} 缺少十六进制数字".to_owned());
                }
                let character =
                    char::from_u32(value).ok_or("\\u{...} 不是合法的 Unicode 标量值")?;
                Ok(EscapeValue::Char(character))
            }
            other => Err(format!("不支持的转义序列 `\\{other}`")),
        }
    }

    fn hex_digit(&mut self) -> Option<u8> {
        let digit = self.peek_char()?.to_digit(16)? as u8;
        self.bump();
        Some(digit)
    }

    fn lex_template_text(&mut self) -> Token {
        let start = self.pos;
        let mut text = String::new();
        loop {
            let Some(character) = self.peek_char() else {
                self.error("模板字符串未闭合", Span::new(start, self.pos));
                self.template_text_mode = false;
                break;
            };
            if character == '`' {
                self.bump();
                self.template_text_mode = false;
                self.buffer.push_back(Token {
                    kind: TokenKind::TemplateEnd,
                    span: Span::new(self.pos - 1, self.pos),
                });
                break;
            }
            if character == '$' && self.peek_char_at(1) == Some('{') {
                self.bump_twice();
                self.template_text_mode = false;
                self.buffer.push_back(Token {
                    kind: TokenKind::TemplateExprStart,
                    span: Span::new(self.pos - 2, self.pos),
                });
                break;
            }
            text.push(character);
            self.bump();
        }
        Token {
            kind: TokenKind::TemplateText(text),
            span: Span::new(start, self.pos),
        }
    }
}

enum Suffix {
    Integer(IntegerSuffix),
    Float(FloatSuffix),
}

enum EscapeValue {
    Char(char),
    Bytes(u8),
}

fn is_ident_start(character: char) -> bool {
    character == '_' || character == '$' || character.is_alphabetic()
}

fn is_ident_continue(character: char) -> bool {
    character == '_' || character == '$' || character.is_alphanumeric()
}

fn is_base_digit(character: char, base: u32) -> bool {
    character.to_digit(base).is_some()
}
