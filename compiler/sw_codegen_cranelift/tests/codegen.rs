use std::path::PathBuf;

use sw_codegen_cranelift::compile_module;
use sw_semantic::analyze;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate 目录")
        .join("sw_semantic")
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn compiles_mir_to_object_file() {
    let result = analyze(&fixture("basic.sw"), None);
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
    assert_eq!(result.modules.len(), 1);
    let bytes = compile_module(&result.modules[0], &result.type_table).expect("编译成功");
    assert!(bytes.len() > 100, "对象文件过小：{} 字节", bytes.len());
    #[cfg(target_os = "windows")]
    {
        // COFF：x86-64 machine = 0x8664（小端）
        assert_eq!(&bytes[0..2], &[0x64, 0x86], "应为 COFF 对象");
    }
    #[cfg(target_os = "linux")]
    {
        assert_eq!(&bytes[0..4], &[0x7F, b'E', b'L', b'F'], "应为 ELF 对象");
    }
}

#[test]
fn compiles_template_and_extern_calls() {
    let source = r#"
        import { println } from "std/io";
        function main(): int {
            println(`value = ${42}`);
            return 0;
        }
    "#;
    let dir = std::env::temp_dir().join("swcc-codegen-test");
    let _ = std::fs::create_dir_all(&dir);
    let entry = dir.join("main.sw");
    std::fs::write(&entry, source).expect("写入测试源码");
    let stdlib = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate 目录")
        .parent()
        .expect("工作区根")
        .join("stdlib");
    let result = analyze(&entry, Some(&stdlib));
    assert!(
        !result.diagnostics.has_errors(),
        "{:?}",
        result.diagnostics.items
    );
    let total_bytes: usize = result
        .modules
        .iter()
        .map(|module| {
            compile_module(module, &result.type_table)
                .expect("编译成功")
                .len()
        })
        .sum();
    assert!(total_bytes > 200);
}
