use std::path::PathBuf;

use sw_common::{Diagnostics, Source};
use sw_frontend::Parser;
use sw_frontend::ast::*;

fn parse(text: &str) -> (Module, Diagnostics) {
    let source = Source::new(PathBuf::from("test.sw"), text.to_owned());
    let mut diagnostics = Diagnostics::new();
    let mut parser = Parser::new(&source, &mut diagnostics);
    let module = parser.parse_module();
    (module, diagnostics)
}

fn single_expr(text: &str) -> Expr {
    let (module, diagnostics) = parse(&format!("function main(): int {{ return {text}; }}"));
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    let ItemKind::Function(function) = &module.items[0].kind else {
        panic!("预期函数");
    };
    let StmtKind::Return(Some(expr)) = &function.body.as_ref().unwrap().statements[0].kind else {
        panic!("预期 return");
    };
    expr.clone()
}

fn dump(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Integer { text, .. } => text.clone(),
        ExprKind::Float { text, .. } => text.clone(),
        ExprKind::Str(value) => format!("str({value})"),
        ExprKind::Bool(value) => value.to_string(),
        ExprKind::Null => "null".to_owned(),
        ExprKind::Ident(ident) => ident.name.clone(),
        ExprKind::Binary { op, left, right } => {
            format!("({} {} {})", dump(left), binop(op), dump(right))
        }
        ExprKind::Unary { op, expr } => format!("({}{})", unop(op), dump(expr)),
        ExprKind::Group(inner) => format!("({})", dump(inner)),
        ExprKind::Call { callee, args } => {
            let args: Vec<String> = args.iter().map(dump).collect();
            format!("{}({})", dump(callee), args.join(","))
        }
        ExprKind::Member { object, name, .. } => format!("{}.{}", dump(object), name.name),
        ExprKind::Index { object, index, .. } => format!("{}[{}]", dump(object), dump(index)),
        ExprKind::Conditional { cond, then, else_ } => {
            format!("({}?{}:{})", dump(cond), dump(then), dump(else_))
        }
        other => format!("{other:?}"),
    }
}

fn binop(op: &BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::Pow => "**",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::Coalesce => "??",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
    }
}

fn unop(op: &UnaryOp) -> &'static str {
    match op {
        UnaryOp::Not => "!",
        UnaryOp::Neg => "-",
        UnaryOp::Pos => "+",
        UnaryOp::BitNot => "~",
        UnaryOp::Inc => "++",
        UnaryOp::Dec => "--",
        UnaryOp::Await => "await ",
    }
}

#[test]
fn parses_function_with_params_defaults_and_generics() {
    let (module, diagnostics) = parse(
        r#"
        function connect<T>(host: string, port: int = 8080): bool where T: Drawable
        {
            return true;
        }
        "#,
    );
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    assert_eq!(module.items.len(), 1);
    let ItemKind::Function(function) = &module.items[0].kind else {
        panic!("预期函数");
    };
    assert_eq!(function.name.name, "connect");
    assert_eq!(function.generics.len(), 1);
    assert_eq!(function.params.len(), 2);
    assert!(function.params[1].default.is_some());
    assert_eq!(function.where_clause.len(), 1);
}

#[test]
fn parses_class_with_members() {
    let (module, diagnostics) = parse(
        r#"
        interface Drawable { draw(): void; }
        class Circle extends Shape implements Drawable
        {
            private radius: float;
            constructor(name: string, radius: float) { super(name); this.radius = radius; }
            override area(): float { return 3.14 * this.radius * this.radius; }
            property name: string { get { return this._name; } set { this._name = value; } }
            static create(): Circle { return new Circle("c", 1); }
        }
        "#,
    );
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    assert_eq!(module.items.len(), 2);
    let ItemKind::Interface(interface) = &module.items[0].kind else {
        panic!("预期接口");
    };
    assert_eq!(interface.methods.len(), 1);
    let ItemKind::Class(class) = &module.items[1].kind else {
        panic!("预期类");
    };
    assert_eq!(class.members.len(), 5);
}

#[test]
fn parses_control_flow_statements() {
    let (module, diagnostics) = parse(
        r#"
        function main(): int
        {
            let total = 0;
            if (total > 0) { total = 1; } else if (total == 0) { total = 2; } else { total = 3; }
            while (total < 10) { total++; }
            for (let i = 0; i < 10; i++) { total += i; }
            for (const item of items) { total += item; }
            switch (total) {
                case 0: total = 1; break;
                case 1:
                case 2: total = 3; break;
                default: total = 0;
            }
            try { risky(); } catch (e: Error) { handle(e); } finally { cleanup(); }
            defer file.close();
            return total;
        }
        "#,
    );
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    let ItemKind::Function(function) = &module.items[0].kind else {
        panic!("预期函数");
    };
    let statements = &function.body.as_ref().unwrap().statements;
    assert_eq!(statements.len(), 9);
}

#[test]
fn expression_precedence_matches_js() {
    assert_eq!(dump(&single_expr("1 + 2 * 3")), "(1 + (2 * 3))");
    assert_eq!(dump(&single_expr("(1 + 2) * 3")), "(((1 + 2)) * 3)");
    assert_eq!(dump(&single_expr("a || b && c")), "(a || (b && c))");
    assert_eq!(dump(&single_expr("2 ** 3 ** 2")), "(2 ** (3 ** 2))");
    assert_eq!(dump(&single_expr("-a + b")), "((-a) + b)");
    assert_eq!(dump(&single_expr("a == b < c")), "(a == (b < c))");
    assert_eq!(dump(&single_expr("a ?? b || c")), "(a ?? (b || c))");
}

#[test]
fn parses_templates_with_interpolation() {
    let (module, diagnostics) = parse(
        r#"
        function greet(name: string): string { return `hi ${name}!`; }
        "#,
    );
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    let ItemKind::Function(function) = &module.items[0].kind else {
        panic!("预期函数");
    };
    let StmtKind::Return(Some(expr)) = &function.body.as_ref().unwrap().statements[0].kind else {
        panic!("预期 return");
    };
    let ExprKind::Template(parts) = &expr.kind else {
        panic!("预期模板字符串，实际 {:?}", expr.kind);
    };
    assert_eq!(parts.len(), 3);
    assert!(matches!(&parts[0], TemplatePart::Text(text) if text == "hi "));
    assert!(matches!(&parts[1], TemplatePart::Expr(_)));
    assert!(matches!(&parts[2], TemplatePart::Text(text) if text == "!"));
}

#[test]
fn parses_lambdas_and_new() {
    let (module, diagnostics) = parse(
        r#"
        function main(): int {
            const double = (x: int) => x * 2;
            const add = (a, b) => a + b;
            const point = new Point(1, 2);
            // 带返回类型注解的 lambda（bug #3 修复）
            const typed = (x: int): int => x * 2;
            const block_typed = (): int => { return 5; };
            return add(1, 2);
        }
        "#,
    );
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    let ItemKind::Function(function) = &module.items[0].kind else {
        panic!("预期函数");
    };
    assert_eq!(function.body.as_ref().unwrap().statements.len(), 6);
}

#[test]
fn parses_object_literals_and_optional_chaining() {
    assert_eq!(dump(&single_expr("obj?.field")), "obj.field");
    assert_eq!(dump(&single_expr("arr?.[0]")), "arr[0]");
    let (module, diagnostics) = parse(
        r#"
        function main(): int {
            const config = { name: "sw", count: 1 };
            const shorthand = { config };
            return config.count;
        }
        "#,
    );
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    let ItemKind::Function(function) = &module.items[0].kind else {
        panic!("预期函数");
    };
    assert_eq!(function.body.as_ref().unwrap().statements.len(), 3);
}

#[test]
fn parses_imports() {
    let (module, diagnostics) = parse(
        r#"
        import { println, print as write } from "std/io";
        import * as fs from "std/fs";
        import "./startup";
        "#,
    );
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    assert_eq!(module.items.len(), 3);
}

#[test]
fn rejects_unsupported_js_keywords() {
    let (_, diagnostics) = parse("function main(): int { var x = 1; return x; }");
    assert!(diagnostics.has_errors());
}

#[test]
fn rejects_missing_semicolon() {
    let (_, diagnostics) = parse("function main(): int { let x = 1 return x; }");
    assert!(diagnostics.has_errors());
}

#[test]
fn rejects_unclosed_block() {
    let (_, diagnostics) = parse("function main(): int {");
    assert!(diagnostics.has_errors());
}

#[test]
fn parses_extern_c_function() {
    let (module, diagnostics) = parse("extern c function native_add(a: i32, b: i32): i32;");
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.items);
    let ItemKind::Function(function) = &module.items[0].kind else {
        panic!("预期函数");
    };
    assert!(function.extern_c);
    assert!(function.body.is_none());
}
