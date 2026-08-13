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
