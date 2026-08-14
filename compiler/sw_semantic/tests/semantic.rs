use std::path::PathBuf;

use sw_semantic::analyze;

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
    assert_eq!(result.modules[0].functions.len(), 2);
    let main = result.modules[0]
        .functions
        .iter()
        .find(|function| function.name.contains("main"))
        .expect("存在 main");
    assert!(main.locals.len() >= 1);
    assert!(!main.body.is_empty());
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
    assert_eq!(result.modules[0].functions.len(), 3);
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
