use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use sw_common::{Diagnostics, Severity, Source};
use sw_frontend::Parser;
use sw_semantic::analyze;

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

    let mut syntax_errors = false;
    for item in &diagnostics.items {
        let severity = match item.severity {
            Severity::Error => "错误",
            Severity::Warning => "警告",
        };
        if item.severity == Severity::Error {
            syntax_errors = true;
        }
        match item.span.map(|span| source.line_col(span.start)) {
            Some((line, column)) => {
                eprintln!("{path}:{line}:{column}: {severity}: {}", item.message);
            }
            None => {
                eprintln!("{path}: {severity}: {}", item.message);
            }
        }
    }
    if syntax_errors {
        eprintln!("语法检查失败：{} 个错误", diagnostics.items.len());
        std::process::exit(1);
    }
    let _ = module;

    let stdlib_dir = env::var("SW_STDLIB").map(PathBuf::from).ok().or_else(|| {
        let candidate = env::current_dir().ok()?.join("stdlib");
        candidate.is_dir().then_some(candidate)
    });
    let result = analyze(Path::new(path), stdlib_dir.as_deref());

    let mut sources: HashMap<PathBuf, Source> = result
        .module_sources
        .iter()
        .map(|(module_path, text)| {
            (
                module_path.clone(),
                Source::new(module_path.clone(), text.clone()),
            )
        })
        .collect();
    sources.insert(
        fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path)),
        source.clone(),
    );

    for item in &result.diagnostics.items {
        let severity = match item.severity {
            Severity::Error => "错误",
            Severity::Warning => "警告",
        };
        let file = item.file.clone().unwrap_or_else(|| PathBuf::from(path));
        let location = item.span.and_then(|span| {
            sources
                .get(&file)
                .map(|file_source| file_source.line_col(span.start))
        });
        match location {
            Some((line, column)) => {
                eprintln!(
                    "{}:{line}:{column}: {severity}: {}",
                    file.display(),
                    item.message
                );
            }
            None => {
                eprintln!("{}: {severity}: {}", file.display(), item.message);
            }
        }
    }
    if result.diagnostics.has_errors() {
        eprintln!("语义检查失败：{} 个错误", result.diagnostics.items.len());
        std::process::exit(1);
    }
    let function_count: usize = result
        .modules
        .iter()
        .map(|module| module.functions.len())
        .sum();
    println!(
        "语义检查通过：{} 个模块，{} 个函数（MIR 已生成）",
        result.modules.len(),
        function_count
    );
}
