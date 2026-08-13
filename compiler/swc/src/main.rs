use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sw_codegen_cranelift::compile_module;
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
    if !matches!(command, "check" | "build" | "run") {
        eprintln!("未知命令 `{command}`；支持 check / build / run");
        std::process::exit(2);
    }
    let Some(path) = args.get(2) else {
        eprintln!("用法: swc {command} <文件.sw> [-o 输出.exe]");
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
    if env::var("SW_DEBUG_MIR").is_ok() {
        eprintln!("{:#?}", result.modules);
    }

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

    let mut semantic_errors = 0;
    for item in &result.diagnostics.items {
        let severity = match item.severity {
            Severity::Error => "错误",
            Severity::Warning => "警告",
        };
        if item.severity == Severity::Error {
            semantic_errors += 1;
        }
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
    if semantic_errors > 0 {
        eprintln!("语义检查失败：{semantic_errors} 个错误");
        std::process::exit(1);
    }
    let function_count: usize = result
        .modules
        .iter()
        .map(|module| module.functions.len())
        .sum();
    if command == "check" {
        println!(
            "语义检查通过：{} 个模块，{} 个函数（MIR 已生成）",
            result.modules.len(),
            function_count
        );
        return;
    }

    // ---- 代码生成与链接 ----
    let output = match parse_output(&args) {
        Some(output) => output,
        None => {
            let stem = Path::new(path)
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| "main".to_owned());
            PathBuf::from(format!("{stem}.exe"))
        }
    };
    let cache_dir = PathBuf::from(".swcache").join("obj");
    if let Err(error) = fs::create_dir_all(&cache_dir) {
        eprintln!("无法创建缓存目录：{error}");
        std::process::exit(2);
    }

    let mut objects = Vec::new();
    for (index, mir) in result.modules.iter().enumerate() {
        let bytes = match compile_module(mir, &result.type_table) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("代码生成失败（模块 {index}）：{}", error.message);
                std::process::exit(1);
            }
        };
        let object_path = cache_dir.join(format!("mod_{index}.obj"));
        if let Err(error) = fs::write(&object_path, bytes) {
            eprintln!("无法写入目标文件：{error}");
            std::process::exit(2);
        }
        objects.push(object_path);
    }

    let clang = find_clang().unwrap_or_else(|| {
        eprintln!(
            "未找到 clang；请设置环境变量 SW_CLANG 指向 clang.exe，或安装 Visual Studio LLVM 工具"
        );
        std::process::exit(2);
    });
    let runtime_c = env::var("SW_RUNTIME_C")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            let candidate = env::current_dir().ok()?.join("runtime").join("runtime.c");
            candidate.is_file().then_some(candidate)
        })
        .unwrap_or_else(|| {
            eprintln!("未找到 runtime/runtime.c");
            std::process::exit(2);
        });
    let runtime_obj = cache_dir.join("runtime.obj");
    if !runtime_obj.exists() {
        let status = Command::new(&clang)
            .args(["-O2", "-c"])
            .arg(&runtime_c)
            .arg("-o")
            .arg(&runtime_obj)
            .status();
        if !status.map(|status| status.success()).unwrap_or(false) {
            eprintln!("运行时编译失败");
            std::process::exit(1);
        }
    }

    let mut link_args = Vec::new();
    for object in &objects {
        link_args.push(object.as_os_str().to_os_string());
    }
    link_args.push(runtime_obj.as_os_str().to_os_string());
    link_args.push("-o".into());
    link_args.push(output.as_os_str().to_os_string());
    let status = Command::new(&clang).args(&link_args).status();
    if !status.map(|status| status.success()).unwrap_or(false) {
        eprintln!("链接失败");
        std::process::exit(1);
    }
    println!("构建成功：{}", output.display());

    if command == "run" {
        let run_path = fs::canonicalize(&output).unwrap_or(output.clone());
        let status = Command::new(&run_path).status();
        match status {
            Ok(status) => {
                let code = status.code().unwrap_or(1);
                std::process::exit(code);
            }
            Err(error) => {
                eprintln!("无法运行 `{}`：{error}", run_path.display());
                std::process::exit(2);
            }
        }
    }
}

fn parse_output(args: &[String]) -> Option<PathBuf> {
    let mut index = 3;
    while index < args.len() {
        if args[index] == "-o" || args[index] == "--output" {
            return args.get(index + 1).map(PathBuf::from);
        }
        index += 1;
    }
    None
}

fn find_clang() -> Option<PathBuf> {
    if let Some(path) = env::var_os("SW_CLANG") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    let candidates = [
        r"C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\Llvm\x64\bin\clang.exe",
        r"C:\Program Files\Microsoft Visual Studio\17\Community\VC\Tools\Llvm\x64\bin\clang.exe",
        r"C:\Program Files\Microsoft Visual Studio\16\Community\VC\Tools\Llvm\x64\bin\clang.exe",
    ];
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
}
