use std::collections::VecDeque;

use sw_common::{Diagnostics, Source, Span};

use crate::ast::*;
use crate::lexer::Lexer;
use crate::token::{Keyword, Token, TokenKind};

/// 递归下降语法分析器：按 02-语法规则.md 把 token 流解析为 AST。
pub struct Parser<'a> {
    lexer: Lexer<'a>,
    lookahead: VecDeque<Token>,
}

struct Checkpoint {
    lexer: crate::lexer::Checkpoint,
    lookahead: VecDeque<Token>,
    diagnostics_len: usize,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a Source, diagnostics: &'a mut Diagnostics) -> Self {
        let lexer = Lexer::new(source, diagnostics);
        Self {
            lexer,
            lookahead: VecDeque::new(),
        }
    }

    pub fn parse_module(&mut self) -> Module {
        let mut items = Vec::new();
        loop {
            if self.at(&TokenKind::Eof) {
                break;
            }
            match self.parse_item() {
                Some(item) => items.push(item),
                None => self.synchronize_item(),
            }
        }
        Module { items }
    }

    // ---------- token 工具 ----------

    fn peek(&mut self) -> Token {
        self.fill_lookahead(1);
        self.lookahead[0].clone()
    }

    fn peek_n(&mut self, n: usize) -> Token {
        self.fill_lookahead(n);
        self.lookahead[n - 1].clone()
    }

    fn fill_lookahead(&mut self, count: usize) {
        while self.lookahead.len() < count {
            let token = self.lexer.next_token();
            self.lookahead.push_back(token);
        }
    }

    fn advance(&mut self) -> Token {
        self.fill_lookahead(1);
        self.lookahead.pop_front().expect("lookahead 非空")
    }

    fn at(&mut self, kind: &TokenKind) -> bool {
        matches!(&self.peek().kind, found if found == kind)
    }

    fn at_keyword(&mut self, keyword: Keyword) -> bool {
        self.peek().is_keyword(keyword)
    }

    fn at_ident(&mut self, name: &str) -> bool {
        self.peek().is_ident(name)
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: &TokenKind, message: &str) -> Result<Token, ()> {
        if self.at(kind) {
            Ok(self.advance())
        } else {
            let span = self.peek().span;
            self.error(message.to_owned(), span);
            Err(())
        }
    }

    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.lexer.error(message, span);
    }

    fn expect_ident(&mut self, context: &str) -> Result<Ident, ()> {
        let token = self.advance();
        match token.kind {
            TokenKind::Ident(name) => Ok(Ident {
                name,
                span: token.span,
            }),
            other => {
                self.error(
                    format!("{context}，预期标识符，实际遇到 {}", describe_kind(&other)),
                    token.span,
                );
                Err(())
            }
        }
    }

    fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            lexer: self.lexer.checkpoint(),
            lookahead: self.lookahead.clone(),
            diagnostics_len: self.lexer.diagnostics_len(),
        }
    }

    fn restore(&mut self, checkpoint: Checkpoint) {
        self.lexer.restore(checkpoint.lexer);
        self.lookahead = checkpoint.lookahead;
        self.lexer.truncate_diagnostics(checkpoint.diagnostics_len);
    }

    // ---------- 顶层 ----------

    fn parse_item(&mut self) -> Option<Item> {
        let mut attributes = Vec::new();
        while self.at(&TokenKind::At) {
            let attribute = self.parse_attribute()?;
            attributes.push(attribute);
        }
        let exported = self.eat(&TokenKind::Keyword(Keyword::Export));
        let start = self.peek().span.start;

        let kind = match &self.peek().kind {
            TokenKind::Keyword(Keyword::Import) => ItemKind::Import(self.parse_import()?),
            TokenKind::Keyword(Keyword::Function) | TokenKind::Keyword(Keyword::Async) => {
                let function = self.parse_function(false)?;
                ItemKind::Function(function)
            }
            TokenKind::Ident(name) if name == "extern" => {
                let function = self.parse_function(false)?;
                ItemKind::Function(function)
            }
            TokenKind::Keyword(Keyword::Struct) => ItemKind::Struct(self.parse_struct()?),
            TokenKind::Keyword(Keyword::Enum) => ItemKind::Enum(self.parse_enum()?),
            TokenKind::Keyword(Keyword::Class) => ItemKind::Class(self.parse_class()?),
            TokenKind::Keyword(Keyword::Interface) => ItemKind::Interface(self.parse_interface()?),
            TokenKind::Keyword(Keyword::Type) => ItemKind::TypeAlias(self.parse_type_alias()?),
            TokenKind::Keyword(Keyword::Let) | TokenKind::Keyword(Keyword::Const) => {
                ItemKind::Variable(self.parse_variable().ok()?)
            }
            TokenKind::Keyword(keyword)
                if matches!(
                    keyword,
                    Keyword::UnsupportedVar
                        | Keyword::UnsupportedUndefined
                        | Keyword::UnsupportedTypeof
                        | Keyword::UnsupportedInstanceof
                        | Keyword::UnsupportedDo
                ) =>
            {
                let keyword = *keyword;
                let span = self.peek().span;
                self.advance();
                self.error(
                    format!(
                        "`{}` 不支持：{}",
                        keyword.as_str(),
                        unsupported_hint(keyword)
                    ),
                    span,
                );
                return None;
            }
            _ => {
                let token = self.peek();
                self.error(
                    format!(
                        "顶层只能出现导入、函数、结构体、枚举、类、接口、类型别名或变量声明，实际遇到 {}",
                        token.describe()
                    ),
                    token.span,
                );
                return None;
            }
        };

        let span = Span::new(start, self.peek().span.start);
        Some(Item {
            attributes,
            exported,
            kind,
            span,
        })
    }

    fn synchronize_item(&mut self) {
        loop {
            match &self.peek().kind {
                TokenKind::Eof => return,
                TokenKind::Keyword(Keyword::Import)
                | TokenKind::Keyword(Keyword::Export)
                | TokenKind::Keyword(Keyword::Function)
                | TokenKind::Keyword(Keyword::Async)
                | TokenKind::Keyword(Keyword::Struct)
                | TokenKind::Keyword(Keyword::Enum)
                | TokenKind::Keyword(Keyword::Class)
                | TokenKind::Keyword(Keyword::Interface)
                | TokenKind::Keyword(Keyword::Type)
                | TokenKind::Keyword(Keyword::Let)
                | TokenKind::Keyword(Keyword::Const)
                | TokenKind::At => return,
                TokenKind::Ident(ident) if ident == "extern" => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn parse_attribute(&mut self) -> Option<Attribute> {
        let start = self.peek().span.start;
        self.advance(); // '@'
        let name = self.expect_ident("属性名").ok()?;
        let mut arguments = Vec::new();
        if self.at(&TokenKind::LParen) {
            self.advance();
            while !self.at(&TokenKind::RParen) {
                let key = self.expect_ident("属性参数名").ok()?;
                let value = if self.eat(&TokenKind::Assign) {
                    self.parse_attribute_value()?
                } else {
                    AttributeValue::Ident(key.clone())
                };
                arguments.push((key, value));
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::RParen, "属性参数缺少 `)`").ok();
        }
        Some(Attribute {
            name,
            arguments,
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_attribute_value(&mut self) -> Option<AttributeValue> {
        let token = self.advance();
        match &token.kind {
            TokenKind::Ident(name) => Some(AttributeValue::Ident(Ident {
                name: name.clone(),
                span: token.span,
            })),
            TokenKind::Str(value) => Some(AttributeValue::String(value.clone())),
            TokenKind::Integer { text, .. } => match text.replace('_', "").parse::<i128>() {
                Ok(value) => Some(AttributeValue::Integer(value)),
                Err(_) => {
                    self.error("属性整数参数越界", token.span);
                    None
                }
            },
            TokenKind::Keyword(Keyword::True) => Some(AttributeValue::Bool(true)),
            TokenKind::Keyword(Keyword::False) => Some(AttributeValue::Bool(false)),
            TokenKind::Keyword(Keyword::Null) => Some(AttributeValue::Null),
            _ => {
                self.error(format!("属性参数值无效：{}", token.describe()), token.span);
                None
            }
        }
    }

    fn parse_import(&mut self) -> Option<ImportDecl> {
        let start = self.peek().span.start;
        self.advance(); // import
        let kind = match &self.peek().kind {
            TokenKind::Str(_) => ImportKind::SideEffect,
            TokenKind::LBrace => {
                self.advance();
                let mut specifiers = Vec::new();
                loop {
                    if self.at(&TokenKind::RBrace) {
                        break;
                    }
                    let name = self.expect_ident("导入名称").ok()?;
                    let alias = if self.at_ident("as") {
                        self.advance();
                        Some(self.expect_ident("导入别名").ok()?)
                    } else {
                        None
                    };
                    specifiers.push(ImportSpecifier { name, alias });
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RBrace, "命名导入缺少 `}`").ok()?;
                if !self.at_ident("from") {
                    let span = self.peek().span;
                    self.error("命名导入缺少 `from`", span);
                    return None;
                }
                self.advance();
                ImportKind::Named(specifiers)
            }
            TokenKind::Star => {
                self.advance();
                if !self.at_ident("as") {
                    let span = self.peek().span;
                    self.error("命名空间导入缺少 `as`", span);
                    return None;
                }
                self.advance();
                let alias = self.expect_ident("命名空间别名").ok()?;
                if !self.at_ident("from") {
                    let span = self.peek().span;
                    self.error("命名空间导入缺少 `from`", span);
                    return None;
                }
                self.advance();
                ImportKind::Namespace(alias)
            }
            _ => {
                let token = self.peek();
                self.error(format!("导入声明无效：{}", token.describe()), token.span);
                return None;
            }
        };

        let path = match &self.peek().kind {
            TokenKind::Str(path) => {
                let path = path.clone();
                self.advance();
                path
            }
            _ => {
                let span = self.peek().span;
                self.error("导入缺少模块路径字符串", span);
                return None;
            }
        };
        self.expect(&TokenKind::Semicolon, "导入声明缺少 `;`").ok();
        Some(ImportDecl {
            kind,
            path,
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_function(&mut self, class_method: bool) -> Option<FunctionDecl> {
        let start = self.peek().span.start;
        let mut async_ = false;
        let mut extern_c = false;
        if self.at_keyword(Keyword::Async) {
            self.advance();
            async_ = true;
        } else if self.at_ident("extern") {
            self.advance();
            extern_c = true;
            if self.at_ident("c") {
                self.advance();
            }
        }
        if !class_method {
            self.expect(
                &TokenKind::Keyword(Keyword::Function),
                "函数声明缺少 `function`",
            )
            .ok();
        } else {
            self.eat(&TokenKind::Keyword(Keyword::Function));
        }
        let name = self.expect_ident("函数名").ok()?;
        let generics = self.parse_generic_parameters().ok()?;
        let params = self.parse_parameter_list().ok()?;
        let return_type = if self.eat(&TokenKind::Colon) {
            Some(self.parse_type().ok()?)
        } else {
            None
        };
        let where_clause = self.parse_where_clause().ok()?;
        let body = if self.eat(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_block().ok()?)
        };
        Some(FunctionDecl {
            async_,
            extern_c,
            name,
            generics,
            params,
            return_type,
            where_clause,
            body,
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_generic_parameters(&mut self) -> Result<Vec<Ident>, ()> {
        let mut generics = Vec::new();
        if !self.at(&TokenKind::Lt) {
            return Ok(generics);
        }
        self.advance();
        loop {
            generics.push(self.expect_ident("泛型参数名")?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            if self.at(&TokenKind::Gt) {
                break;
            }
        }
        self.expect(&TokenKind::Gt, "泛型参数缺少 `>`").ok();
        Ok(generics)
    }

    fn parse_parameter_list(&mut self) -> Result<Vec<Param>, ()> {
        let mut params = Vec::new();
        self.expect(&TokenKind::LParen, "参数列表缺少 `(`")?;
        if self.at(&TokenKind::RParen) {
            self.advance();
            return Ok(params);
        }
        loop {
            let start = self.peek().span.start;
            let rest = self.eat(&TokenKind::DotDotDot);
            let name = self.expect_ident("参数名")?;
            self.expect(&TokenKind::Colon, "参数缺少类型标注 `:`")?;
            let ty = self.parse_type()?;
            let default = if self.eat(&TokenKind::Assign) {
                Some(self.parse_expression().map_err(|_| ())?)
            } else {
                None
            };
            params.push(Param {
                rest,
                name,
                ty,
                default,
                span: Span::new(start, self.peek().span.start),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            if self.at(&TokenKind::RParen) {
                break;
            }
        }
        self.expect(&TokenKind::RParen, "参数列表缺少 `)`").ok();
        Ok(params)
    }

    fn parse_where_clause(&mut self) -> Result<Vec<WhereConstraint>, ()> {
        let mut constraints = Vec::new();
        if !self.at_ident("where") {
            return Ok(constraints);
        }
        self.advance();
        loop {
            let start = self.peek().span.start;
            let name = self.expect_ident("约束类型参数")?;
            self.expect(&TokenKind::Colon, "约束缺少 `:`")?;
            let bound = self.parse_type()?;
            constraints.push(WhereConstraint {
                name,
                bound,
                span: Span::new(start, self.peek().span.start),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Ok(constraints)
    }

    fn parse_type(&mut self) -> Result<TypeRef, ()> {
        let start = self.peek().span.start;
        let mut segments = Vec::new();
        loop {
            let name = self.expect_ident("类型名")?;
            let generics = self.parse_type_generics()?;
            segments.push(TypeSegment { name, generics });
            if !self.eat(&TokenKind::Dot) {
                break;
            }
        }
        let mut suffixes = Vec::new();
        loop {
            if self.at(&TokenKind::LBracket) && matches!(self.peek_n(2).kind, TokenKind::RBracket) {
                self.advance();
                self.advance();
                suffixes.push(TypeSuffix::Array);
            } else if self.eat(&TokenKind::Question) {
                suffixes.push(TypeSuffix::Nullable);
            } else {
                break;
            }
        }
        Ok(TypeRef {
            segments,
            suffixes,
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_type_generics(&mut self) -> Result<Vec<TypeRef>, ()> {
        let mut types = Vec::new();
        if !self.at(&TokenKind::Lt) {
            return Ok(types);
        }
        self.advance();
        loop {
            types.push(self.parse_type()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            if self.at(&TokenKind::Gt) {
                break;
            }
        }
        self.expect(&TokenKind::Gt, "泛型类型缺少 `>`")?;
        Ok(types)
    }

    fn parse_block(&mut self) -> Result<Block, ()> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::LBrace, "代码块缺少 `{`")?;
        let mut statements = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            match self.parse_statement() {
                Some(statement) => statements.push(statement),
                None => self.synchronize_statement(),
            }
        }
        self.expect(&TokenKind::RBrace, "代码块缺少 `}`").ok();
        Ok(Block {
            statements,
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn synchronize_statement(&mut self) {
        loop {
            match &self.peek().kind {
                TokenKind::Eof | TokenKind::RBrace => return,
                TokenKind::Semicolon => {
                    self.advance();
                    return;
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn parse_statement(&mut self) -> Option<Stmt> {
        let start = self.peek().span.start;
        let kind = match &self.peek().kind {
            TokenKind::LBrace => {
                let block = self.parse_block().ok()?;
                StmtKind::Block(block)
            }
            TokenKind::Keyword(Keyword::Let) | TokenKind::Keyword(Keyword::Const) => {
                let variable = self.parse_variable().ok()?;
                self.expect(&TokenKind::Semicolon, "变量声明缺少 `;`").ok();
                StmtKind::Variable(variable)
            }
            TokenKind::Keyword(Keyword::If) => self.parse_if().ok()?,
            TokenKind::Keyword(Keyword::While) => self.parse_while().ok()?,
            TokenKind::Keyword(Keyword::For) => self.parse_for().ok()?,
            TokenKind::Keyword(Keyword::Switch) => self.parse_switch().ok()?,
            TokenKind::Keyword(Keyword::Try) => self.parse_try().ok()?,
            TokenKind::Keyword(Keyword::Throw) => {
                self.advance();
                let expr = self.parse_expression().ok()?;
                self.expect(&TokenKind::Semicolon, "throw 语句缺少 `;`")
                    .ok();
                StmtKind::Throw(expr)
            }
            TokenKind::Keyword(Keyword::Defer) => {
                self.advance();
                let expr = self.parse_expression().ok()?;
                self.expect(&TokenKind::Semicolon, "defer 语句缺少 `;`")
                    .ok();
                StmtKind::Defer(expr)
            }
            TokenKind::Keyword(Keyword::Break) => {
                self.advance();
                self.expect(&TokenKind::Semicolon, "break 缺少 `;`").ok();
                StmtKind::Break
            }
            TokenKind::Keyword(Keyword::Continue) => {
                self.advance();
                self.expect(&TokenKind::Semicolon, "continue 缺少 `;`").ok();
                StmtKind::Continue
            }
            TokenKind::Keyword(Keyword::Return) => {
                self.advance();
                let value = if self.at(&TokenKind::Semicolon) {
                    None
                } else {
                    Some(self.parse_expression().ok()?)
                };
                self.expect(&TokenKind::Semicolon, "return 缺少 `;`").ok();
                StmtKind::Return(value)
            }
            TokenKind::Semicolon => {
                self.advance();
                StmtKind::Empty
            }
            TokenKind::Keyword(keyword)
                if matches!(
                    keyword,
                    Keyword::UnsupportedVar
                        | Keyword::UnsupportedUndefined
                        | Keyword::UnsupportedTypeof
                        | Keyword::UnsupportedInstanceof
                        | Keyword::UnsupportedDo
                ) =>
            {
                let keyword = *keyword;
                let span = self.peek().span;
                self.advance();
                self.error(
                    format!(
                        "`{}` 不支持：{}",
                        keyword.as_str(),
                        unsupported_hint(keyword)
                    ),
                    span,
                );
                return None;
            }
            _ => {
                let expr = self.parse_expression().ok()?;
                self.expect(&TokenKind::Semicolon, "表达式语句缺少 `;`")
                    .ok();
                StmtKind::Expr(expr)
            }
        };
        Some(Stmt {
            kind,
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_variable(&mut self) -> Result<VariableDecl, ()> {
        let start = self.peek().span.start;
        let kind = match &self.peek().kind {
            TokenKind::Keyword(Keyword::Let) => VarKind::Let,
            TokenKind::Keyword(Keyword::Const) => VarKind::Const,
            _ => {
                let span = self.peek().span;
                self.error("预期 `let` 或 `const`", span);
                return Err(());
            }
        };
        self.advance();
        let name = self.expect_ident("变量名")?;
        let ty = if self.eat(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let init = if self.eat(&TokenKind::Assign) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        if ty.is_none() && init.is_none() {
            let span = Span::new(start, self.peek().span.start);
            self.error("变量声明需要类型标注或初始化表达式", span);
        }
        Ok(VariableDecl {
            kind,
            name,
            ty,
            init,
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_if(&mut self) -> Result<StmtKind, ()> {
        self.advance(); // if
        self.expect(&TokenKind::LParen, "if 缺少 `(`")?;
        let cond = self.parse_expression()?;
        self.expect(&TokenKind::RParen, "if 条件缺少 `)`")?;
        let then = Box::new(self.parse_statement().ok_or(())?);
        let else_ = if self.at_keyword(Keyword::Else) {
            self.advance();
            Some(Box::new(self.parse_statement().ok_or(())?))
        } else {
            None
        };
        Ok(StmtKind::If { cond, then, else_ })
    }

    fn parse_while(&mut self) -> Result<StmtKind, ()> {
        self.advance(); // while
        self.expect(&TokenKind::LParen, "while 缺少 `(`")?;
        let cond = self.parse_expression()?;
        self.expect(&TokenKind::RParen, "while 条件缺少 `)`")?;
        let body = Box::new(self.parse_statement().ok_or(())?);
        Ok(StmtKind::While { cond, body })
    }

    fn parse_for(&mut self) -> Result<StmtKind, ()> {
        self.advance(); // for
        self.expect(&TokenKind::LParen, "for 缺少 `(`")?;

        if matches!(
            &self.peek().kind,
            TokenKind::Keyword(Keyword::Let) | TokenKind::Keyword(Keyword::Const)
        ) {
            let start = self.peek().span.start;
            let kind = if self.at_keyword(Keyword::Let) {
                VarKind::Let
            } else {
                VarKind::Const
            };
            self.advance();
            let name = self.expect_ident("for-of 变量名")?;
            if self.at_keyword(Keyword::Of) {
                self.advance();
                let iterable = self.parse_expression()?;
                self.expect(&TokenKind::RParen, "for-of 缺少 `)`")?;
                let body = Box::new(self.parse_statement().ok_or(())?);
                return Ok(StmtKind::ForEach {
                    kind,
                    name,
                    iterable,
                    body,
                });
            }
            let ty = if self.eat(&TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            let init = if self.eat(&TokenKind::Assign) {
                Some(self.parse_expression()?)
            } else {
                None
            };
            self.expect(&TokenKind::Semicolon, "for 初始化后缺少 `;`")?;
            let cond = if self.at(&TokenKind::Semicolon) {
                None
            } else {
                Some(self.parse_expression()?)
            };
            self.expect(&TokenKind::Semicolon, "for 条件后缺少 `;`")?;
            let update = if self.at(&TokenKind::RParen) {
                None
            } else {
                Some(self.parse_expression()?)
            };
            self.expect(&TokenKind::RParen, "for 缺少 `)`")?;
            let body = Box::new(self.parse_statement().ok_or(())?);
            return Ok(StmtKind::For {
                init: Some(ForInit::Variable(VariableDecl {
                    kind,
                    name,
                    ty,
                    init,
                    span: Span::new(start, self.peek().span.start),
                })),
                cond,
                update,
                body,
            });
        }

        let init = if self.at(&TokenKind::Semicolon) {
            None
        } else {
            Some(ForInit::Expr(self.parse_expression()?))
        };
        self.expect(&TokenKind::Semicolon, "for 初始化后缺少 `;`")?;
        let cond = if self.at(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(&TokenKind::Semicolon, "for 条件后缺少 `;`")?;
        let update = if self.at(&TokenKind::RParen) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(&TokenKind::RParen, "for 缺少 `)`")?;
        let body = Box::new(self.parse_statement().ok_or(())?);
        Ok(StmtKind::For {
            init,
            cond,
            update,
            body,
        })
    }

    fn parse_switch(&mut self) -> Result<StmtKind, ()> {
        self.advance(); // switch
        self.expect(&TokenKind::LParen, "switch 缺少 `(`")?;
        let value = self.parse_expression()?;
        self.expect(&TokenKind::RParen, "switch 缺少 `)`")?;
        self.expect(&TokenKind::LBrace, "switch 缺少 `{`")?;

        let mut cases = Vec::new();
        let mut default = None;
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            if self.at_keyword(Keyword::Case) {
                self.advance();
                let case_value = self.parse_expression()?;
                self.expect(&TokenKind::Colon, "case 后缺少 `:`")?;
                let mut body = Vec::new();
                while !self.at_keyword(Keyword::Case)
                    && !self.at_keyword(Keyword::Default)
                    && !self.at(&TokenKind::RBrace)
                    && !self.at(&TokenKind::Eof)
                {
                    if let Some(statement) = self.parse_statement() {
                        body.push(statement);
                    } else {
                        self.synchronize_statement();
                    }
                }
                cases.push(SwitchCase {
                    value: case_value,
                    body,
                });
            } else if self.at_keyword(Keyword::Default) {
                self.advance();
                self.expect(&TokenKind::Colon, "default 后缺少 `:`")?;
                let mut body = Vec::new();
                while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
                    if let Some(statement) = self.parse_statement() {
                        body.push(statement);
                    } else {
                        self.synchronize_statement();
                    }
                }
                default = Some(body);
            } else {
                let token = self.peek();
                self.error(
                    format!(
                        "switch 内预期 `case` 或 `default`，实际遇到 {}",
                        token.describe()
                    ),
                    token.span,
                );
                self.advance();
            }
        }
        self.expect(&TokenKind::RBrace, "switch 缺少 `}`")?;
        Ok(StmtKind::Switch {
            value,
            cases,
            default,
        })
    }

    fn parse_try(&mut self) -> Result<StmtKind, ()> {
        self.advance(); // try
        let body = self.parse_block()?;
        let mut catches = Vec::new();
        while self.at_keyword(Keyword::Catch) {
            self.advance();
            self.expect(&TokenKind::LParen, "catch 缺少 `(`")?;
            let name = self.expect_ident("catch 参数名")?;
            let ty = if self.eat(&TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            self.expect(&TokenKind::RParen, "catch 缺少 `)`")?;
            let catch_body = self.parse_block()?;
            catches.push(CatchClause {
                name,
                ty,
                body: catch_body,
            });
        }
        if catches.is_empty() {
            let span = self.peek().span;
            self.error("try 语句缺少 catch 或 finally", span);
        }
        let finally = if self.at_keyword(Keyword::Finally) {
            self.advance();
            Some(self.parse_block()?)
        } else {
            None
        };
        Ok(StmtKind::Try {
            body,
            catches,
            finally,
        })
    }

    // ---------- 结构体 / 枚举 / 类 / 接口 / 类型别名 ----------

    fn parse_struct(&mut self) -> Option<StructDecl> {
        let start = self.peek().span.start;
        self.advance(); // struct
        let name = self.expect_ident("结构体名").ok()?;
        let generics = self.parse_generic_parameters().ok()?;
        let where_clause = self.parse_where_clause().ok()?;
        self.expect(&TokenKind::LBrace, "结构体缺少 `{`").ok()?;
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            fields.push(self.parse_field()?);
        }
        self.expect(&TokenKind::RBrace, "结构体缺少 `}`").ok();
        Some(StructDecl {
            name,
            generics,
            where_clause,
            fields,
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_field(&mut self) -> Option<FieldDecl> {
        let start = self.peek().span.start;
        let modifiers = self.parse_member_modifiers();
        let mut attributes = Vec::new();
        while self.at(&TokenKind::At) {
            attributes.push(self.parse_attribute()?);
        }
        let name = self.expect_ident("字段名").ok()?;
        self.expect(&TokenKind::Colon, "字段缺少类型标注 `:`")
            .ok()?;
        let ty = self.parse_type().ok()?;
        let default = if self.eat(&TokenKind::Assign) {
            self.parse_expression().ok()
        } else {
            None
        };
        self.expect(&TokenKind::Semicolon, "字段声明缺少 `;`").ok();
        Some(FieldDecl {
            modifiers,
            attributes,
            name,
            ty,
            default,
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_member_modifiers(&mut self) -> Vec<MemberModifier> {
        let mut modifiers = Vec::new();
        loop {
            let modifier = match &self.peek().kind {
                TokenKind::Keyword(Keyword::Public) => Some(MemberModifier::Public),
                TokenKind::Keyword(Keyword::Private) => Some(MemberModifier::Private),
                TokenKind::Keyword(Keyword::Protected) => Some(MemberModifier::Protected),
                TokenKind::Keyword(Keyword::Internal) => Some(MemberModifier::Internal),
                TokenKind::Keyword(Keyword::Static) => Some(MemberModifier::Static),
                TokenKind::Keyword(Keyword::Virtual) => Some(MemberModifier::Virtual),
                TokenKind::Keyword(Keyword::Override) => Some(MemberModifier::Override),
                TokenKind::Keyword(Keyword::Final) => Some(MemberModifier::Final),
                _ => None,
            };
            match modifier {
                Some(modifier) => {
                    self.advance();
                    modifiers.push(modifier);
                }
                None => return modifiers,
            }
        }
    }

    fn parse_enum(&mut self) -> Option<EnumDecl> {
        let start = self.peek().span.start;
        self.advance(); // enum
        let name = self.expect_ident("枚举名").ok()?;
        self.expect(&TokenKind::LBrace, "枚举缺少 `{`").ok()?;
        let mut members = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            let member_start = self.peek().span.start;
            let member_name = self.expect_ident("枚举成员名").ok()?;
            let value = if self.eat(&TokenKind::Assign) {
                self.parse_expression().ok()
            } else {
                None
            };
            members.push(EnumMember {
                name: member_name,
                value,
                span: Span::new(member_start, self.peek().span.start),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            if self.at(&TokenKind::RBrace) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace, "枚举缺少 `}`").ok();
        Some(EnumDecl {
            name,
            members,
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_class(&mut self) -> Option<ClassDecl> {
        let start = self.peek().span.start;
        let final_ = self.eat(&TokenKind::Keyword(Keyword::Final));
        self.expect(&TokenKind::Keyword(Keyword::Class), "类声明缺少 `class`")
            .ok()?;
        let name = self.expect_ident("类名").ok()?;
        let generics = self.parse_generic_parameters().ok()?;
        let extends = if self.at_keyword(Keyword::Extends) {
            self.advance();
            Some(self.parse_type().ok()?)
        } else {
            None
        };
        let implements = if self.at_keyword(Keyword::Implements) {
            self.advance();
            let mut types = vec![self.parse_type().ok()?];
            while self.eat(&TokenKind::Comma) {
                types.push(self.parse_type().ok()?);
            }
            types
        } else {
            Vec::new()
        };
        let where_clause = self.parse_where_clause().ok()?;
        self.expect(&TokenKind::LBrace, "类缺少 `{`").ok()?;
        let mut members = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            members.push(self.parse_class_member()?);
        }
        self.expect(&TokenKind::RBrace, "类缺少 `}`").ok();
        Some(ClassDecl {
            final_,
            name,
            generics,
            extends,
            implements,
            where_clause,
            members,
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_class_member(&mut self) -> Option<ClassMember> {
        let start = self.peek().span.start;
        let modifiers = self.parse_member_modifiers();
        match &self.peek().kind {
            TokenKind::Keyword(Keyword::Constructor) => {
                self.advance();
                let params = self.parse_parameter_list().ok()?;
                let body = self.parse_block().ok()?;
                Some(ClassMember::Constructor(ConstructorDecl {
                    params,
                    body,
                    span: Span::new(start, self.peek().span.start),
                }))
            }
            TokenKind::Keyword(Keyword::Destructor) => {
                self.advance();
                self.expect(&TokenKind::LParen, "destructor 缺少 `(`")
                    .ok()?;
                self.expect(&TokenKind::RParen, "destructor 缺少 `)`")
                    .ok()?;
                let body = self.parse_block().ok()?;
                Some(ClassMember::Destructor(DestructorDecl {
                    body,
                    span: Span::new(start, self.peek().span.start),
                }))
            }
            TokenKind::Keyword(Keyword::Property) => {
                self.advance();
                let name = self.expect_ident("属性名").ok()?;
                self.expect(&TokenKind::Colon, "属性缺少类型标注 `:`")
                    .ok()?;
                let ty = self.parse_type().ok()?;
                self.expect(&TokenKind::LBrace, "属性缺少 `{`").ok()?;
                let mut get = None;
                let mut set = None;
                while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
                    if self.at_ident("get") {
                        self.advance();
                        get = Some(self.parse_block().ok()?);
                    } else if self.at_ident("set") {
                        self.advance();
                        set = Some(self.parse_block().ok()?);
                    } else {
                        let token = self.peek();
                        self.error(
                            format!("属性内预期 `get` 或 `set`，实际遇到 {}", token.describe()),
                            token.span,
                        );
                        self.advance();
                    }
                }
                self.expect(&TokenKind::RBrace, "属性缺少 `}`").ok()?;
                Some(ClassMember::Property(PropertyDecl {
                    name,
                    ty,
                    get,
                    set,
                    span: Span::new(start, self.peek().span.start),
                }))
            }
            _ => {
                if self.at(&TokenKind::Semicolon) {
                    self.advance();
                    let span = Span::new(start, self.peek().span.start);
                    self.error("类成员不能只有分号", span);
                    return None;
                }
                // 方法或字段：方法带参数列表，字段带 `:` 类型标注。
                let function_or_async = self.at_keyword(Keyword::Function)
                    || self.at_keyword(Keyword::Async)
                    || self.at_ident("extern");
                if function_or_async {
                    let function = self.parse_function(true)?;
                    Some(ClassMember::Method(function))
                } else {
                    let name = self.expect_ident("类成员名").ok()?;
                    if self.at(&TokenKind::LParen) {
                        let params = self.parse_parameter_list().ok()?;
                        let return_type = if self.eat(&TokenKind::Colon) {
                            Some(self.parse_type().ok()?)
                        } else {
                            None
                        };
                        let body = if self.eat(&TokenKind::Semicolon) {
                            None
                        } else {
                            Some(self.parse_block().ok()?)
                        };
                        Some(ClassMember::Method(FunctionDecl {
                            async_: false,
                            extern_c: false,
                            name,
                            generics: Vec::new(),
                            params,
                            return_type,
                            where_clause: Vec::new(),
                            body,
                            span: Span::new(start, self.peek().span.start),
                        }))
                    } else {
                        self.expect(&TokenKind::Colon, "字段缺少类型标注 `:`")
                            .ok()?;
                        let ty = self.parse_type().ok()?;
                        let default = if self.eat(&TokenKind::Assign) {
                            self.parse_expression().ok()
                        } else {
                            None
                        };
                        self.expect(&TokenKind::Semicolon, "字段声明缺少 `;`")
                            .ok()?;
                        Some(ClassMember::Field(FieldDecl {
                            modifiers,
                            attributes: Vec::new(),
                            name,
                            ty,
                            default,
                            span: Span::new(start, self.peek().span.start),
                        }))
                    }
                }
            }
        }
    }

    fn parse_interface(&mut self) -> Option<InterfaceDecl> {
        let start = self.peek().span.start;
        self.advance(); // interface
        let name = self.expect_ident("接口名").ok()?;
        let generics = self.parse_generic_parameters().ok()?;
        let extends = if self.at_keyword(Keyword::Extends) {
            self.advance();
            let mut types = vec![self.parse_type().ok()?];
            while self.eat(&TokenKind::Comma) {
                types.push(self.parse_type().ok()?);
            }
            types
        } else {
            Vec::new()
        };
        let where_clause = self.parse_where_clause().ok()?;
        self.expect(&TokenKind::LBrace, "接口缺少 `{`").ok()?;
        let mut methods = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            self.parse_member_modifiers();
            if self.at(&TokenKind::Semicolon) {
                self.advance();
                continue;
            }
            let function = self.parse_function(true)?;
            if function.body.is_some() {
                self.error("接口方法不能有函数体", function.span);
            }
            methods.push(function);
        }
        self.expect(&TokenKind::RBrace, "接口缺少 `}`").ok();
        Some(InterfaceDecl {
            name,
            generics,
            extends,
            where_clause,
            methods,
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_type_alias(&mut self) -> Option<TypeAliasDecl> {
        let start = self.peek().span.start;
        self.advance(); // type
        let name = self.expect_ident("类型别名").ok()?;
        self.expect(&TokenKind::Assign, "类型别名缺少 `=`").ok()?;
        let ty = self.parse_type().ok()?;
        self.expect(&TokenKind::Semicolon, "类型别名缺少 `;`").ok();
        Some(TypeAliasDecl {
            name,
            ty,
            span: Span::new(start, self.peek().span.start),
        })
    }

    // ---------- 表达式 ----------

    fn parse_expression(&mut self) -> Result<Expr, ()> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr, ()> {
        let start = self.peek().span.start;
        let target = self.parse_conditional()?;
        let Some(op) = self.try_assignment_op() else {
            return Ok(target);
        };
        self.advance();
        let value = self.parse_assignment()?;
        Ok(Expr {
            kind: ExprKind::Assign {
                op,
                target: Box::new(target),
                value: Box::new(value),
            },
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn try_assignment_op(&mut self) -> Option<AssignOp> {
        let op = match &self.peek().kind {
            TokenKind::Assign => AssignOp::Assign,
            TokenKind::PlusAssign => AssignOp::Add,
            TokenKind::MinusAssign => AssignOp::Sub,
            TokenKind::StarAssign => AssignOp::Mul,
            TokenKind::SlashAssign => AssignOp::Div,
            TokenKind::PercentAssign => AssignOp::Rem,
            TokenKind::AmpAssign => AssignOp::BitAnd,
            TokenKind::PipeAssign => AssignOp::BitOr,
            TokenKind::CaretAssign => AssignOp::BitXor,
            TokenKind::ShlAssign => AssignOp::Shl,
            TokenKind::ShrAssign => AssignOp::Shr,
            TokenKind::CoalesceAssign => AssignOp::Coalesce,
            _ => return None,
        };
        Some(op)
    }

    fn parse_conditional(&mut self) -> Result<Expr, ()> {
        let start = self.peek().span.start;
        let cond = self.parse_nullish()?;
        if !self.at(&TokenKind::Question) {
            return Ok(cond);
        }
        self.advance();
        let then = self.parse_assignment()?;
        self.expect(&TokenKind::Colon, "三元表达式缺少 `:`")?;
        let else_ = self.parse_assignment()?;
        Ok(Expr {
            kind: ExprKind::Conditional {
                cond: Box::new(cond),
                then: Box::new(then),
                else_: Box::new(else_),
            },
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_nullish(&mut self) -> Result<Expr, ()> {
        self.parse_binary_level(
            &[TokenKind::QuestionQuestion],
            BinaryOp::Coalesce,
            Self::parse_logical_or,
        )
    }

    fn parse_logical_or(&mut self) -> Result<Expr, ()> {
        self.parse_binary_level(
            &[TokenKind::PipePipe],
            BinaryOp::Or,
            Self::parse_logical_and,
        )
    }

    fn parse_logical_and(&mut self) -> Result<Expr, ()> {
        self.parse_binary_level(&[TokenKind::AmpAmp], BinaryOp::And, Self::parse_bitwise_or)
    }

    fn parse_bitwise_or(&mut self) -> Result<Expr, ()> {
        self.parse_binary_level(&[TokenKind::Pipe], BinaryOp::BitOr, Self::parse_bitwise_xor)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Expr, ()> {
        self.parse_binary_level(
            &[TokenKind::Caret],
            BinaryOp::BitXor,
            Self::parse_bitwise_and,
        )
    }

    fn parse_bitwise_and(&mut self) -> Result<Expr, ()> {
        self.parse_binary_level(&[TokenKind::Amp], BinaryOp::BitAnd, Self::parse_equality)
    }

    fn parse_equality(&mut self) -> Result<Expr, ()> {
        self.parse_binary_level(
            &[TokenKind::Eq, TokenKind::Ne],
            BinaryOp::Eq,
            Self::parse_relational,
        )
    }

    fn parse_relational(&mut self) -> Result<Expr, ()> {
        self.parse_binary_level(
            &[TokenKind::Lt, TokenKind::Le, TokenKind::Gt, TokenKind::Ge],
            BinaryOp::Lt,
            Self::parse_shift,
        )
    }

    fn parse_shift(&mut self) -> Result<Expr, ()> {
        self.parse_binary_level(
            &[TokenKind::Shl, TokenKind::Shr],
            BinaryOp::Shl,
            Self::parse_additive,
        )
    }

    fn parse_additive(&mut self) -> Result<Expr, ()> {
        self.parse_binary_level(
            &[TokenKind::Plus, TokenKind::Minus],
            BinaryOp::Add,
            Self::parse_multiplicative,
        )
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ()> {
        self.parse_binary_level(
            &[TokenKind::Star, TokenKind::Slash, TokenKind::Percent],
            BinaryOp::Mul,
            Self::parse_unary,
        )
    }

    fn parse_binary_level(
        &mut self,
        operators: &[TokenKind],
        base_op: BinaryOp,
        next: fn(&mut Self) -> Result<Expr, ()>,
    ) -> Result<Expr, ()> {
        let start = self.peek().span.start;
        let mut left = next(self)?;
        loop {
            let Some(op) = self.binary_op_for(operators, base_op) else {
                break;
            };
            self.advance();
            let right = next(self)?;
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: Span::new(start, self.peek().span.start),
            };
        }
        Ok(left)
    }

    fn binary_op_for(&mut self, operators: &[TokenKind], base_op: BinaryOp) -> Option<BinaryOp> {
        let found = &self.peek().kind;
        if !operators.iter().any(|operator| operator == found) {
            return None;
        }
        Some(match found {
            TokenKind::Eq => BinaryOp::Eq,
            TokenKind::Ne => BinaryOp::Ne,
            TokenKind::Lt => BinaryOp::Lt,
            TokenKind::Le => BinaryOp::Le,
            TokenKind::Gt => BinaryOp::Gt,
            TokenKind::Ge => BinaryOp::Ge,
            TokenKind::Shl => BinaryOp::Shl,
            TokenKind::Shr => BinaryOp::Shr,
            TokenKind::Plus => BinaryOp::Add,
            TokenKind::Minus => BinaryOp::Sub,
            TokenKind::Star => BinaryOp::Mul,
            TokenKind::Slash => BinaryOp::Div,
            TokenKind::Percent => BinaryOp::Rem,
            _ => base_op,
        })
    }

    fn parse_unary(&mut self) -> Result<Expr, ()> {
        let start = self.peek().span.start;
        let op = match &self.peek().kind {
            TokenKind::Bang => UnaryOp::Not,
            TokenKind::Minus => UnaryOp::Neg,
            TokenKind::Plus => UnaryOp::Pos,
            TokenKind::Tilde => UnaryOp::BitNot,
            TokenKind::PlusPlus => UnaryOp::Inc,
            TokenKind::MinusMinus => UnaryOp::Dec,
            TokenKind::Keyword(Keyword::Await) => UnaryOp::Await,
            _ => return self.parse_power(),
        };
        self.advance();
        let expr = self.parse_unary()?;
        Ok(Expr {
            kind: ExprKind::Unary {
                op,
                expr: Box::new(expr),
            },
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_power(&mut self) -> Result<Expr, ()> {
        let start = self.peek().span.start;
        let base = self.parse_postfix()?;
        if !self.at(&TokenKind::StarStar) {
            return Ok(base);
        }
        self.advance();
        let exponent = self.parse_unary()?;
        Ok(Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Pow,
                left: Box::new(base),
                right: Box::new(exponent),
            },
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_postfix(&mut self) -> Result<Expr, ()> {
        let start = self.peek().span.start;
        let mut expr = self.parse_primary()?;
        loop {
            match &self.peek().kind {
                TokenKind::LParen => {
                    let args = self.parse_arguments()?;
                    expr = Expr {
                        kind: ExprKind::Call {
                            callee: Box::new(expr),
                            args,
                        },
                        span: Span::new(start, self.peek().span.start),
                    };
                }
                TokenKind::Dot => {
                    self.advance();
                    let name = self.expect_ident("成员名")?;
                    expr = Expr {
                        kind: ExprKind::Member {
                            object: Box::new(expr),
                            name,
                            optional: false,
                        },
                        span: Span::new(start, self.peek().span.start),
                    };
                }
                TokenKind::QuestionDot => {
                    self.advance();
                    if self.at(&TokenKind::LBracket) {
                        self.advance();
                        let index = self.parse_expression()?;
                        self.expect(&TokenKind::RBracket, "可选索引缺少 `]`")?;
                        expr = Expr {
                            kind: ExprKind::Index {
                                object: Box::new(expr),
                                index: Box::new(index),
                                optional: true,
                            },
                            span: Span::new(start, self.peek().span.start),
                        };
                    } else {
                        let name = self.expect_ident("成员名")?;
                        expr = Expr {
                            kind: ExprKind::Member {
                                object: Box::new(expr),
                                name,
                                optional: true,
                            },
                            span: Span::new(start, self.peek().span.start),
                        };
                    }
                }
                TokenKind::LBracket => {
                    self.advance();
                    let index = self.parse_expression()?;
                    self.expect(&TokenKind::RBracket, "索引缺少 `]`")?;
                    expr = Expr {
                        kind: ExprKind::Index {
                            object: Box::new(expr),
                            index: Box::new(index),
                            optional: false,
                        },
                        span: Span::new(start, self.peek().span.start),
                    };
                }
                TokenKind::PlusPlus => {
                    self.advance();
                    expr = Expr {
                        kind: ExprKind::Postfix {
                            expr: Box::new(expr),
                            op: PostfixOp::Inc,
                        },
                        span: Span::new(start, self.peek().span.start),
                    };
                }
                TokenKind::MinusMinus => {
                    self.advance();
                    expr = Expr {
                        kind: ExprKind::Postfix {
                            expr: Box::new(expr),
                            op: PostfixOp::Dec,
                        },
                        span: Span::new(start, self.peek().span.start),
                    };
                }
                TokenKind::Ident(name) if name == "as" => {
                    self.advance();
                    let ty = self.parse_type()?;
                    expr = Expr {
                        kind: ExprKind::Cast {
                            expr: Box::new(expr),
                            ty,
                        },
                        span: Span::new(start, self.peek().span.start),
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_arguments(&mut self) -> Result<Vec<Expr>, ()> {
        self.expect(&TokenKind::LParen, "参数列表缺少 `(`")?;
        let mut arguments = Vec::new();
        if self.at(&TokenKind::RParen) {
            self.advance();
            return Ok(arguments);
        }
        loop {
            arguments.push(self.parse_assignment()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            if self.at(&TokenKind::RParen) {
                break;
            }
        }
        self.expect(&TokenKind::RParen, "参数列表缺少 `)`")?;
        Ok(arguments)
    }

    fn parse_primary(&mut self) -> Result<Expr, ()> {
        let start = self.peek().span.start;
        if self.at(&TokenKind::LParen) {
            return self.parse_paren_or_lambda(start);
        }
        let token = self.advance();
        let kind = match token.kind {
            TokenKind::Integer { text, suffix } => ExprKind::Integer { text, suffix },
            TokenKind::Float { text, suffix } => ExprKind::Float { text, suffix },
            TokenKind::Str(value) => ExprKind::Str(value),
            TokenKind::Char(character) => ExprKind::Char(character),
            TokenKind::Keyword(Keyword::True) => ExprKind::Bool(true),
            TokenKind::Keyword(Keyword::False) => ExprKind::Bool(false),
            TokenKind::Keyword(Keyword::Null) => ExprKind::Null,
            TokenKind::Keyword(Keyword::This) => ExprKind::This,
            TokenKind::Keyword(Keyword::Super) => ExprKind::Super,
            TokenKind::Ident(name) => ExprKind::Ident(Ident {
                name,
                span: token.span,
            }),
            TokenKind::TemplateStart => {
                return self.parse_template(token.span);
            }
            TokenKind::Keyword(Keyword::New) => return self.parse_new(start),
            TokenKind::LBracket => {
                let mut elements = Vec::new();
                if !self.at(&TokenKind::RBracket) {
                    loop {
                        elements.push(self.parse_assignment()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                        if self.at(&TokenKind::RBracket) {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::RBracket, "数组字面量缺少 `]`")?;
                ExprKind::Array(elements)
            }
            TokenKind::LBrace => {
                let mut fields = Vec::new();
                if !self.at(&TokenKind::RBrace) {
                    loop {
                        let key = match &self.peek().kind {
                            TokenKind::Ident(name) => {
                                let name = name.clone();
                                let span = self.peek().span;
                                self.advance();
                                ObjectKey::Ident(Ident { name, span })
                            }
                            TokenKind::Str(value) => {
                                let value = value.clone();
                                self.advance();
                                ObjectKey::Str(value)
                            }
                            _ => {
                                let token = self.peek();
                                self.error(
                                    format!("对象字段名无效：{}", token.describe()),
                                    token.span,
                                );
                                return Err(());
                            }
                        };
                        let value = if self.eat(&TokenKind::Colon) {
                            self.parse_assignment()?
                        } else {
                            let name = match &key {
                                ObjectKey::Ident(ident) => ident.clone(),
                                ObjectKey::Str(value) => Ident {
                                    name: value.clone(),
                                    span: Span::empty(start),
                                },
                            };
                            Expr {
                                kind: ExprKind::Ident(name),
                                span: Span::empty(start),
                            }
                        };
                        fields.push(ObjectField { key, value });
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                        if self.at(&TokenKind::RBrace) {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::RBrace, "对象字面量缺少 `}`")?;
                ExprKind::Object(fields)
            }
            TokenKind::LParen => unreachable!("LParen 已在 parse_primary 开头处理"),
            other => {
                self.error(
                    format!("预期表达式，实际遇到 {}", describe_kind(&other)),
                    token.span,
                );
                return Err(());
            }
        };
        Ok(Expr {
            kind,
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_new(&mut self, start: usize) -> Result<Expr, ()> {
        let ty = self.parse_type()?;
        let args = self.parse_arguments()?;
        Ok(Expr {
            kind: ExprKind::New { ty, args },
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn parse_paren_or_lambda(&mut self, start: usize) -> Result<Expr, ()> {
        let checkpoint = self.checkpoint();
        if let Some(lambda) = self.try_parse_lambda()? {
            return Ok(lambda);
        }
        self.restore(checkpoint);
        self.advance(); // '('
        let inner = self.parse_expression()?;
        self.expect(&TokenKind::RParen, "分组表达式缺少 `)`")?;
        Ok(Expr {
            kind: ExprKind::Group(Box::new(inner)),
            span: Span::new(start, self.peek().span.start),
        })
    }

    fn try_parse_lambda(&mut self) -> Result<Option<Expr>, ()> {
        // 假定 '(' 尚未消费；失败时调用方恢复 checkpoint。
        let start = self.peek().span.start;
        self.advance(); // '('
        let mut params = Vec::new();
        if self.at(&TokenKind::RParen) {
            self.advance();
            if !self.at(&TokenKind::FatArrow) {
                return Ok(None);
            }
            self.advance();
            let body = self.parse_lambda_body()?;
            return Ok(Some(Expr {
                kind: ExprKind::Lambda { params, body },
                span: Span::new(start, self.peek().span.start),
            }));
        }
        loop {
            let Some(name) = self.try_ident() else {
                return Ok(None);
            };
            let ty = if self.eat(&TokenKind::Colon) {
                match self.try_type() {
                    Some(ty) => Some(ty),
                    None => return Ok(None),
                }
            } else {
                None
            };
            params.push(LambdaParam { name, ty });
            if self.eat(&TokenKind::Comma) {
                continue;
            }
            if self.at(&TokenKind::RParen) {
                self.advance();
                break;
            }
            return Ok(None);
        }
        if !self.at(&TokenKind::FatArrow) {
            return Ok(None);
        }
        self.advance();
        let body = self.parse_lambda_body()?;
        Ok(Some(Expr {
            kind: ExprKind::Lambda { params, body },
            span: Span::new(start, self.peek().span.start),
        }))
    }

    fn try_ident(&mut self) -> Option<Ident> {
        match &self.peek().kind {
            TokenKind::Ident(name) => {
                let name = name.clone();
                let span = self.peek().span;
                self.advance();
                Some(Ident { name, span })
            }
            _ => None,
        }
    }

    fn try_type(&mut self) -> Option<TypeRef> {
        let checkpoint = self.checkpoint();
        match self.parse_type() {
            Ok(ty) => Some(ty),
            Err(()) => {
                self.restore(checkpoint);
                None
            }
        }
    }

    fn parse_lambda_body(&mut self) -> Result<LambdaBody, ()> {
        if self.at(&TokenKind::LBrace) {
            Ok(LambdaBody::Block(self.parse_block()?))
        } else {
            Ok(LambdaBody::Expr(Box::new(self.parse_assignment()?)))
        }
    }

    fn parse_template(&mut self, start: Span) -> Result<Expr, ()> {
        let mut parts = Vec::new();
        loop {
            match &self.peek().kind {
                TokenKind::TemplateText(text) => {
                    let text = text.clone();
                    self.advance();
                    parts.push(TemplatePart::Text(text));
                }
                TokenKind::TemplateExprStart => {
                    self.advance();
                    let expr = self.parse_expression()?;
                    self.expect(&TokenKind::RBrace, "模板插值缺少 `}`")?;
                    self.lexer.resume_template();
                    parts.push(TemplatePart::Expr(expr));
                }
                TokenKind::TemplateEnd => {
                    self.advance();
                    break;
                }
                _ => {
                    let span = self.peek().span;
                    self.error("模板字符串未闭合", span);
                    break;
                }
            }
        }
        Ok(Expr {
            kind: ExprKind::Template(parts),
            span: start.merge(self.peek().span),
        })
    }
}

fn describe_kind(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Ident(name) => format!("标识符 `{name}`"),
        TokenKind::Keyword(keyword) => format!("关键字 `{}`", keyword.as_str()),
        TokenKind::Eof => "文件结束".to_owned(),
        other => format!("{:?}", other),
    }
}

fn unsupported_hint(keyword: Keyword) -> &'static str {
    match keyword {
        Keyword::UnsupportedVar => "请使用 `let` 或 `const`",
        Keyword::UnsupportedUndefined => "空值请使用 `null`",
        Keyword::UnsupportedTypeof => "Sw 是静态类型语言，不需要运行时类型检查",
        Keyword::UnsupportedInstanceof => "类型判断请使用类型系统与显式转换",
        Keyword::UnsupportedDo => "请使用 `while`",
        _ => "不支持",
    }
}
