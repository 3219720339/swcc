use std::path::PathBuf;

use sw_semantic::{Type, analyze};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn checks_basic_program_and_generates_mir() {
    let result = analyze(&fixture("basic.sw"), None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
    assert_eq!(result.modules.len(), 1);
    // 2 个用户函数 + 1 个 sw_global_init（运行时字符串全局初始化）。
    assert_eq!(result.modules[0].functions.len(), 3);
    let main = result.modules[0]
        .functions
        .iter()
        .find(|function| function.name.contains("main"))
        .expect("存在 main");
    assert!(main.locals.len() >= 1);
    assert!(!main.body.is_empty());
}

#[test]
fn preserves_outer_captures_in_block_closures_with_loops() {
    let result = analyze(&fixture("closure-loop.sw"), None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
    let hidden = result.modules[0]
        .functions
        .iter()
        .find(|function| function.name.starts_with("sw_closure_"))
        .expect("存在闭包隐藏函数");
    assert_eq!(hidden.ret, Type::Int);
}

#[test]
fn checks_classes_inheritance_and_methods() {
    let result = analyze(&fixture("classes.sw"), None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
    assert_eq!(result.type_table.classes.len(), 2);
    let function_count = result.modules[0].functions.len();
    assert!(function_count >= 4);
}

#[test]
fn checks_generics_with_inference() {
    let result = analyze(&fixture("generics.sw"), None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
}

#[test]
fn resolves_overloads() {
    let result = analyze(&fixture("overloads.sw"), None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
    // 3 个用户函数 + 1 个 sw_global_init。
    assert_eq!(result.modules[0].functions.len(), 4);
}

#[test]
fn checks_struct_literals() {
    let result = analyze(&fixture("structs.sw"), None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
}

#[test]
fn checks_expr_lowering_with_pow_and_struct() {
    let result = analyze(&fixture("exprs.sw"), None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
    let mut found_assign = false;
    let mut found_postfix = false;
    let mut found_pow = false;
    let mut found_struct = false;
    for module in &result.modules {
        for function in &module.functions {
            for statement in &function.body {
                collect_expr_kinds(
                    statement,
                    &mut found_assign,
                    &mut found_postfix,
                    &mut found_pow,
                    &mut found_struct,
                );
            }
        }
    }
    assert!(found_assign, "应有赋值表达式节点");
    assert!(found_postfix, "应有后缀 ++/-- 节点");
    assert!(found_pow, "应有幂内建调用");
    assert!(found_struct, "应有 struct 字面量节点");
}

#[test]
fn checks_optional_chain() {
    let result = analyze(&fixture("optional.sw"), None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
}

#[test]
fn checks_interfaces_and_vtable_dispatch() {
    let result = analyze(&fixture("interface.sw"), None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
    assert!(
        result
            .type_table
            .class_interfaces
            .values()
            .any(|list| !list.is_empty()),
        "应记录类实现的接口"
    );
}

#[test]
fn checks_generic_struct_and_class() {
    let result = analyze(&fixture("generic-types.sw"), None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
    assert!(
        !result.type_table.generic_struct_instances.is_empty(),
        "应实例化泛型 struct"
    );
    assert!(
        !result.type_table.generic_class_instances.is_empty(),
        "应实例化泛型 class"
    );
}

#[test]
fn checks_string_methods_and_content_compare() {
    let result = analyze(&fixture("string-methods.sw"), None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
}

#[test]
fn checks_main_with_args() {
    let result = analyze(&fixture("process.sw"), None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
}

#[test]
fn checks_io_and_error_handling_extras() {
    let stdlib = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate 目录")
        .parent()
        .expect("工作区根")
        .join("stdlib");
    let result = analyze(&fixture("io-more.sw"), Some(&stdlib));
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
}

#[test]
fn checks_dir_and_file_management() {
    let stdlib = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate 目录")
        .parent()
        .expect("工作区根")
        .join("stdlib");
    let result = analyze(&fixture("dir-more.sw"), Some(&stdlib));
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
}

#[test]
fn checks_format_and_math_extras() {
    let stdlib = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate 目录")
        .parent()
        .expect("工作区根")
        .join("stdlib");
    let result = analyze(&fixture("format-more.sw"), Some(&stdlib));
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
}

#[test]
fn checks_varargs_format_and_new_import_forms() {
    let stdlib = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate 目录")
        .parent()
        .expect("工作区根")
        .join("stdlib");
    let result = analyze(&fixture("varargs-import.sw"), Some(&stdlib));
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
    let mut found_varargs = false;
    for module in &result.modules {
        for function in &module.functions {
            use sw_semantic::mir::{MirExpr, MirStmtKind};
            fn visit_expr(expr: &MirExpr, found: &mut bool) {
                if matches!(expr, MirExpr::VarArgs(_)) {
                    *found = true;
                }
                match expr {
                    MirExpr::VarArgs(items) => {
                        for (_, item) in items {
                            visit_expr(item, found);
                        }
                    }
                    MirExpr::Call { args, .. } => {
                        for arg in args {
                            visit_expr(arg, found);
                        }
                    }
                    MirExpr::Array { items, .. } => {
                        for item in items {
                            visit_expr(item, found);
                        }
                    }
                    MirExpr::Binary { left, right, .. } => {
                        visit_expr(left, found);
                        visit_expr(right, found);
                    }
                    MirExpr::Unary { expr: inner, .. } | MirExpr::Cast { expr: inner, .. } => {
                        visit_expr(inner, found);
                    }
                    MirExpr::Assign { value, .. } => visit_expr(value, found),
                    MirExpr::Select {
                        cond, then, else_, ..
                    } => {
                        visit_expr(cond, found);
                        visit_expr(then, found);
                        visit_expr(else_, found);
                    }
                    MirExpr::Field { object, .. } => visit_expr(object, found),
                    MirExpr::Index { object, index, .. } => {
                        visit_expr(object, found);
                        visit_expr(index, found);
                    }
                    MirExpr::Len { object, .. } => visit_expr(object, found),
                    MirExpr::Struct { fields, .. } => {
                        for (_, field) in fields {
                            visit_expr(field, found);
                        }
                    }
                    MirExpr::ClosureNew { captures, .. } => {
                        for capture in captures {
                            visit_expr(capture, found);
                        }
                    }
                    MirExpr::New { args, .. } => {
                        for arg in args {
                            visit_expr(arg, found);
                        }
                    }
                    _ => {}
                }
            }
            fn visit_stmt(stmt: &sw_semantic::MirStmt, found: &mut bool) {
                match &stmt.kind {
                    MirStmtKind::VarDecl {
                        init: Some(expr), ..
                    }
                    | MirStmtKind::Return(Some(expr))
                    | MirStmtKind::Expr(expr) => visit_expr(expr, found),
                    MirStmtKind::VarDecl { .. } | MirStmtKind::Return(None) => {}
                    MirStmtKind::Assign { value, .. } => visit_expr(value, found),
                    MirStmtKind::If { cond, then, else_ } => {
                        visit_expr(cond, found);
                        for stmt in then {
                            visit_stmt(stmt, found);
                        }
                        for stmt in else_ {
                            visit_stmt(stmt, found);
                        }
                    }
                    MirStmtKind::While { cond, body } => {
                        visit_expr(cond, found);
                        for stmt in body {
                            visit_stmt(stmt, found);
                        }
                    }
                    MirStmtKind::Break | MirStmtKind::Continue => {}
                }
            }
            for statement in &function.body {
                visit_stmt(statement, &mut found_varargs);
            }
        }
    }
    assert!(found_varargs, "期望生成 VarArgs MIR 节点");
}

fn collect_expr_kinds(
    statement: &sw_semantic::MirStmt,
    found_assign: &mut bool,
    found_postfix: &mut bool,
    found_pow: &mut bool,
    found_struct: &mut bool,
) {
    use sw_semantic::mir::{MirExpr, MirStmtKind};
    fn visit_expr(
        expr: &MirExpr,
        found_assign: &mut bool,
        found_postfix: &mut bool,
        found_pow: &mut bool,
        found_struct: &mut bool,
    ) {
        match expr {
            MirExpr::Assign { value, .. } => {
                *found_assign = true;
                visit_expr(value, found_assign, found_postfix, found_pow, found_struct);
            }
            MirExpr::Postfix { .. } => *found_postfix = true,
            MirExpr::Struct { fields, .. } => {
                *found_struct = true;
                for (_, field) in fields {
                    visit_expr(field, found_assign, found_postfix, found_pow, found_struct);
                }
            }
            MirExpr::Call { callee, args } => {
                if let sw_semantic::MirCallee::Intrinsic { name } = callee {
                    if name == "pow_f64" || name == "pow_i64" {
                        *found_pow = true;
                    }
                }
                for arg in args {
                    visit_expr(arg, found_assign, found_postfix, found_pow, found_struct);
                }
            }
            MirExpr::Unary { expr, .. }
            | MirExpr::Cast { expr, .. }
            | MirExpr::Len { object: expr, .. }
            | MirExpr::Field { object: expr, .. }
            | MirExpr::Index { object: expr, .. } => {
                visit_expr(expr, found_assign, found_postfix, found_pow, found_struct);
            }
            MirExpr::Binary { left, right, .. } => {
                visit_expr(left, found_assign, found_postfix, found_pow, found_struct);
                visit_expr(right, found_assign, found_postfix, found_pow, found_struct);
            }
            MirExpr::Select { cond, then, else_ } => {
                visit_expr(cond, found_assign, found_postfix, found_pow, found_struct);
                visit_expr(then, found_assign, found_postfix, found_pow, found_struct);
                visit_expr(else_, found_assign, found_postfix, found_pow, found_struct);
            }
            MirExpr::Array { items, .. } => {
                for item in items {
                    visit_expr(item, found_assign, found_postfix, found_pow, found_struct);
                }
            }
            MirExpr::New { args, .. } => {
                for arg in args {
                    visit_expr(arg, found_assign, found_postfix, found_pow, found_struct);
                }
            }
            MirExpr::ClosureNew { captures, .. } => {
                for capture in captures {
                    visit_expr(
                        capture,
                        found_assign,
                        found_postfix,
                        found_pow,
                        found_struct,
                    );
                }
            }
            _ => {}
        }
    }
    match &statement.kind {
        MirStmtKind::VarDecl {
            init: Some(init), ..
        } => {
            visit_expr(init, found_assign, found_postfix, found_pow, found_struct);
        }
        MirStmtKind::Assign { target, value } => {
            visit_expr(value, found_assign, found_postfix, found_pow, found_struct);
            let _ = target;
        }
        MirStmtKind::Return(Some(value)) => {
            visit_expr(value, found_assign, found_postfix, found_pow, found_struct);
        }
        MirStmtKind::Expr(expr) => {
            visit_expr(expr, found_assign, found_postfix, found_pow, found_struct);
        }
        _ => {}
    }
}

#[test]
fn reports_type_and_name_errors() {
    let result = analyze(&fixture("errors.sw"), None);
    assert!(result.diagnostics.has_errors());
    let messages: Vec<&str> = result
        .diagnostics
        .items
        .iter()
        .map(|item| item.message.as_str())
        .collect();
    assert!(
        messages.iter().any(|message| message.contains("不能赋给")),
        "{messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("未定义的名称")),
        "{messages:?}"
    );
}

#[test]
fn resolves_relative_imports() {
    let result = analyze(&fixture("imports.sw"), None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
    assert_eq!(result.modules.len(), 2);
}

#[test]
fn reports_generic_interface_bound_without_type_args() {
    let source = r#"
        interface Container<T> { get(): T; }
        class IntBox implements Container<int> {
            value: int;
            constructor(value: int) { this.value = value; }
            get(): int { return this.value; }
        }
        function read_raw<T>(container: T): void where T: Container {
            return;
        }
        function main(): int { return 0; }
    "#;
    let dir = std::env::temp_dir().join("swcc-semantic-test");
    let _ = std::fs::create_dir_all(&dir);
    let entry = dir.join("main.sw");
    std::fs::write(&entry, source).expect("写入测试源码");
    let result = analyze(&entry, None);
    assert!(
        result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
    let messages: Vec<&str> = result
        .diagnostics
        .items
        .iter()
        .map(|item| item.message.as_str())
        .collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("泛型接口约束必须带类型实参")),
        "{messages:?}"
    );
}

#[test]
fn reports_missing_interface_method_on_generic_class() {
    let source = r#"
        interface Container<T> { get(): T; set(value: T): void; }
        class BadBox<T> implements Container<T> {
            value: T;
            constructor(value: T) { this.value = value; }
            get(): T { return this.value; }
        }
        function main(): int { return 0; }
    "#;
    let dir = std::env::temp_dir().join("swcc-semantic-test-missing");
    let _ = std::fs::create_dir_all(&dir);
    let entry = dir.join("main.sw");
    std::fs::write(&entry, source).expect("写入测试源码");
    let result = analyze(&entry, None);
    assert!(
        result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
    let messages: Vec<&str> = result
        .diagnostics
        .items
        .iter()
        .map(|item| item.message.as_str())
        .collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("未实现接口") && m.contains("set")),
        "{messages:?}"
    );
}

#[test]
fn reports_incompatible_interface_method_signature() {
    let source = r#"
        interface Container<T> { get(): T; }
        class Wrong<T> implements Container<T> {
            value: T;
            constructor(value: T) { this.value = value; }
            get(): string { return "x"; }
        }
        function main(): int { return 0; }
    "#;
    let dir = std::env::temp_dir().join("swcc-semantic-test-wrongsig");
    let _ = std::fs::create_dir_all(&dir);
    let entry = dir.join("main.sw");
    std::fs::write(&entry, source).expect("写入测试源码");
    let result = analyze(&entry, None);
    assert!(
        result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
    let messages: Vec<&str> = result
        .diagnostics
        .items
        .iter()
        .map(|item| item.message.as_str())
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("签名与接口")),
        "{messages:?}"
    );
}

#[test]
fn accepts_nested_generic_class_instantiation() {
    let source = r#"
        class Box<T> {
            value: T;
            constructor(value: T) { this.value = value; }
            get(): T { return this.value; }
        }
        function main(): int {
            const nb = new Box<Box<int>>(new Box<int>(5));
            return nb.get().get();
        }
    "#;
    let dir = std::env::temp_dir().join("swcc-semantic-test-nested");
    let _ = std::fs::create_dir_all(&dir);
    let entry = dir.join("main.sw");
    std::fs::write(&entry, source).expect("写入测试源码");
    let result = analyze(&entry, None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
}

#[test]
fn accepts_generic_class_in_generic_function_signature() {
    let source = r#"
        class Box<T> {
            value: T;
            constructor(value: T) { this.value = value; }
            get(): T { return this.value; }
        }
        struct Pair<A, B> {
            first: A;
            second: B;
        }
        function make<T>(x: T): Box<T> {
            return new Box<T>(x);
        }
        function read_box<T>(b: Box<T>): T {
            return b.get();
        }
        function make_pair<A, B>(a: A, b: B): Pair<A, B> {
            const p: Pair<A, B> = { first: a, second: b };
            return p;
        }
        function main(): int {
            const b = make(42);
            const v: int = read_box(b);
            const p = make_pair(1, "one");
            return b.get() + v + p.first - 85;
        }
    "#;
    let dir = std::env::temp_dir().join("swcc-semantic-test-gen-sig");
    let _ = std::fs::create_dir_all(&dir);
    let entry = dir.join("main.sw");
    std::fs::write(&entry, source).expect("写入测试源码");
    let result = analyze(&entry, None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
}
