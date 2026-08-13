use std::env;
use std::fs;
use std::path::PathBuf;

use sw_common::{Diagnostics, Severity, Source};
use sw_frontend::Parser;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() == 2 && (args[1] == "--version" || args[1] == "-V") {
        println!("swc 0.1.0");
        return;
    }
    let command = args.get(1).map(String::as_str).unwrap_or("check");
    if command != "check" {
        eprintln!("未知命令 `{command}`；当前支持 `check`（swc check <文件.sw>）");
        std::process::exit(2);
    }
    let Some(path) = args.get(2) else {
        eprintln!("用法: swc check <文件.sw>");
        std::process::exit(2);
    };
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("无法读取 `{path}`：{error}");
            std::process::exit(2);
        }
    };
    let source = Source::new(PathBuf::from(path), text);
    let mut diagnostics = Diagnostics::new();
    let mut parser = Parser::new(&source, &mut diagnostics);
    let module = parser.parse_module();

    for item in &diagnostics.items {
        let severity = match item.severity {
            Severity::Error => "错误",
            Severity::Warning => "警告",
        };
        match item.span.map(|span| source.line_col(span.start)) {
            Some((line, column)) => {
                eprintln!("{path}:{line}:{column}: {severity}: {}", item.message);
            }
            None => {
                eprintln!("{path}: {severity}: {}", item.message);
            }
        }
    }
    if diagnostics.has_errors() {
        eprintln!("语法检查失败：{} 个错误", diagnostics.items.len());
        std::process::exit(1);
    }
    println!("语法检查通过：{} 个顶层项", module.items.len());
}
