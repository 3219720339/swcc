use std::path::PathBuf;

use sw_common::{Diagnostics, Source};
use sw_frontend::{Lexer, TokenKind};

fn lex(text: &str) -> (Vec<TokenKind>, Diagnostics) {
    let source = Source::new(PathBuf::from("test.sw"), text.to_owned());
    let mut diagnostics = Diagnostics::new();
    let mut lexer = Lexer::new(&source, &mut diagnostics);
    let mut kinds = Vec::new();
    loop {
        let token = lexer.next_token();
        if token.kind == TokenKind::Eof {
            break;
        }
        kinds.push(token.kind);
    }
    (kinds, diagnostics)
}

#[test]
fn lexes_integer_literals_with_bases_and_suffixes() {
    let (kinds, diagnostics) = lex("1 0xFF 0b1010 0o755 1_000_000 42u8 7i64");
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    assert_eq!(kinds.len(), 7);
    assert!(matches!(&kinds[0], TokenKind::Integer { text, suffix: None } if text == "1"));
    assert!(matches!(&kinds[1], TokenKind::Integer { text, suffix: None } if text == "0xFF"));
    assert!(matches!(&kinds[2], TokenKind::Integer { text, suffix: None } if text == "0b1010"));
    assert!(matches!(&kinds[3], TokenKind::Integer { text, suffix: None } if text == "0o755"));
    assert!(matches!(&kinds[4], TokenKind::Integer { text, suffix: None } if text == "1_000_000"));
    assert!(
        matches!(&kinds[5], TokenKind::Integer { text, suffix: Some(sw_frontend::IntegerSuffix::U8) } if text == "42")
    );
    assert!(
        matches!(&kinds[6], TokenKind::Integer { text, suffix: Some(sw_frontend::IntegerSuffix::I64) } if text == "7")
    );
}

#[test]
fn lexes_float_literals() {
    let (kinds, diagnostics) = lex("1.5 1e10 2.0f32 3.14e-2f64");
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    assert_eq!(kinds.len(), 4);
    assert!(matches!(&kinds[0], TokenKind::Float { text, suffix: None } if text == "1.5"));
    assert!(matches!(&kinds[1], TokenKind::Float { text, suffix: None } if text == "1e10"));
    assert!(
        matches!(&kinds[2], TokenKind::Float { text, suffix: Some(sw_frontend::FloatSuffix::F32) } if text == "2.0")
    );
    assert!(
        matches!(&kinds[3], TokenKind::Float { text, suffix: Some(sw_frontend::FloatSuffix::F64) } if text == "3.14e-2")
    );
}

#[test]
fn lexes_strings_and_chars() {
    let (kinds, diagnostics) = lex(r#""hi" 'a' 'ab' "\u{4E2D}\n" "\xE4\xB8\xAD""#);
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    assert_eq!(kinds.len(), 5);
    assert_eq!(kinds[0], TokenKind::Str("hi".to_owned()));
    assert_eq!(kinds[1], TokenKind::Char('a'));
    assert_eq!(kinds[2], TokenKind::Str("ab".to_owned()));
    assert_eq!(kinds[3], TokenKind::Str("中\n".to_owned()));
    assert_eq!(kinds[4], TokenKind::Str("中".to_owned()));
}

#[test]
fn lexes_operators_by_longest_match() {
    let (kinds, diagnostics) = lex("a+++b x ??= 1 y **= 2 z <<= 3 w... 4");
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    assert_eq!(kinds[1], TokenKind::PlusPlus);
    assert_eq!(kinds[2], TokenKind::Plus);
    assert_eq!(kinds[5], TokenKind::CoalesceAssign);
    assert_eq!(kinds[8], TokenKind::StarStar);
    assert_eq!(kinds[9], TokenKind::Assign);
    assert_eq!(kinds[12], TokenKind::ShlAssign);
    assert_eq!(kinds[15], TokenKind::DotDotDot);
}

#[test]
fn lexes_keywords_and_identifiers() {
    let (kinds, diagnostics) = lex("let const function class 数量");
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    assert_eq!(kinds[0], TokenKind::Keyword(sw_frontend::Keyword::Let));
    assert_eq!(kinds[1], TokenKind::Keyword(sw_frontend::Keyword::Const));
    assert_eq!(kinds[2], TokenKind::Keyword(sw_frontend::Keyword::Function));
    assert_eq!(kinds[3], TokenKind::Keyword(sw_frontend::Keyword::Class));
    assert_eq!(kinds[4], TokenKind::Ident("数量".to_owned()));
}

#[test]
fn lexes_nested_block_comments_and_line_comments() {
    let (kinds, diagnostics) = lex("1 // 行注释\n 2 /* 外 /* 内 */ 外 */ 3");
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    assert_eq!(kinds.len(), 3);
}

#[test]
fn lexes_template_parts_with_interpolation() {
    let source = Source::new(PathBuf::from("test.sw"), "`a${x}b`".to_owned());
    let mut diagnostics = Diagnostics::new();
    let mut lexer = Lexer::new(&source, &mut diagnostics);
    let mut kinds = Vec::new();
    loop {
        let token = lexer.next_token();
        match token.kind {
            TokenKind::Eof => break,
            TokenKind::RBrace => {
                kinds.push(TokenKind::RBrace);
                lexer.resume_template();
            }
            kind => kinds.push(kind),
        }
    }
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    assert_eq!(
        kinds,
        vec![
            TokenKind::TemplateStart,
            TokenKind::TemplateText("a".to_owned()),
            TokenKind::TemplateExprStart,
            TokenKind::Ident("x".to_owned()),
            TokenKind::RBrace,
            TokenKind::TemplateText("b".to_owned()),
            TokenKind::TemplateEnd,
        ]
    );
}

#[test]
fn lexes_template_escapes_like_strings() {
    // `a\nb\t\u{4E2D}\xE4\xB8\xAD\`c\\d`
    let source = Source::new(
        PathBuf::from("test.sw"),
        "`a\\nb\\t\\u{4E2D}\\xE4\\xB8\\xAD\\`c\\\\d`".to_owned(),
    );
    let mut diagnostics = Diagnostics::new();
    let mut lexer = Lexer::new(&source, &mut diagnostics);
    let mut kinds = Vec::new();
    loop {
        let token = lexer.next_token();
        match token.kind {
            TokenKind::Eof => break,
            kind => kinds.push(kind),
        }
    }
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    assert_eq!(
        kinds,
        vec![
            TokenKind::TemplateStart,
            TokenKind::TemplateText("a\nb\t中中`c\\d".to_owned()),
            TokenKind::TemplateEnd,
        ]
    );
}

#[test]
fn lexes_template_escapes_around_interpolation() {
    // `a\nb${x}c\td`
    let source = Source::new(PathBuf::from("test.sw"), "`a\\nb${x}c\\td`".to_owned());
    let mut diagnostics = Diagnostics::new();
    let mut lexer = Lexer::new(&source, &mut diagnostics);
    let mut kinds = Vec::new();
    loop {
        let token = lexer.next_token();
        match token.kind {
            TokenKind::Eof => break,
            TokenKind::RBrace => {
                kinds.push(TokenKind::RBrace);
                lexer.resume_template();
            }
            kind => kinds.push(kind),
        }
    }
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    assert_eq!(
        kinds,
        vec![
            TokenKind::TemplateStart,
            TokenKind::TemplateText("a\nb".to_owned()),
            TokenKind::TemplateExprStart,
            TokenKind::Ident("x".to_owned()),
            TokenKind::RBrace,
            TokenKind::TemplateText("c\td".to_owned()),
            TokenKind::TemplateEnd,
        ]
    );
}

#[test]
fn reports_unsupported_escape_in_template() {
    let (_, diagnostics) = lex("`a\\qb`");
    assert!(diagnostics.has_errors());
}

#[test]
fn reports_unterminated_string() {
    let (_, diagnostics) = lex(r#""未闭合"#);
    assert!(diagnostics.has_errors());
}

#[test]
fn reports_bad_underscore_in_number() {
    let (_, diagnostics) = lex("1__2");
    assert!(diagnostics.has_errors());
}

#[test]
fn reports_digit_followed_by_identifier() {
    let (_, diagnostics) = lex("123abc");
    assert!(diagnostics.has_errors());
}
