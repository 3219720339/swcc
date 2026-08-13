use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sw_codegen_cranelift::compile_module_for_target;
use sw_common::{Diagnostics, Severity, Source};
use sw_frontend::Parser;
use sw_semantic::analyze;

struct BuildOptions {
    output: Option<PathBuf>,
    target: String,
    emit_object: Option<PathBuf>,
}

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
        eprintln!(
            "用法: swc {command} <文件.sw> [-o 输出] [--target <triple>] [--emit-object <目标文件>]"
        );
        std::process::exit(2);
    };
    let options = parse_options(&args);
    if command == "run" && options.target != default_target() {
        eprintln!("`run` 只支持宿主目标；当前目标 {}", options.target);
        std::process::exit(2);
    }

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

    let stdlib_dir = env::var("SW_STDLIB")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            let candidate = env::current_exe().ok()?.parent()?.join("stdlib");
            candidate.is_dir().then_some(candidate)
        })
        .or_else(|| {
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
    if command == "check" {
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
        return;
    }

    // ---- 代码生成（Cranelift，按目标 triple） ----
    let cache_dir = PathBuf::from(".swcache").join("obj");
    if let Err(error) = fs::create_dir_all(&cache_dir) {
        eprintln!("无法创建缓存目录：{error}");
        std::process::exit(2);
    }
    let extension = if target_family(&options.target) == "windows" {
        "obj"
    } else {
        "o"
    };
    let mut objects = Vec::new();
    for (index, mir) in result.modules.iter().enumerate() {
        let bytes = match compile_module_for_target(mir, &result.type_table, &options.target) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!(
                    "代码生成失败（模块 {index}，目标 {}）：{}",
                    options.target, error.message
                );
                std::process::exit(1);
            }
        };
        let object_path = cache_dir.join(format!("mod_{index}.{extension}"));
        if let Err(error) = fs::write(&object_path, bytes) {
            eprintln!("无法写入目标文件：{error}");
            std::process::exit(2);
        }
        objects.push(object_path);
    }
    if let Some(emit_path) = &options.emit_object {
        if let Some(first) = objects.first() {
            if let Err(error) = fs::copy(first, emit_path) {
                eprintln!("无法写出目标文件：{error}");
                std::process::exit(2);
            }
            println!("目标文件已生成：{}", emit_path.display());
        }
        return;
    }

    let output = options.output.clone().unwrap_or_else(|| {
        let stem = Path::new(path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "main".to_owned());
        PathBuf::from(format!("{stem}{}", executable_suffix(&options.target)))
    });

    match target_family(&options.target) {
        "windows" => link_windows(&options.target, &objects, &output),
        "linux" => link_linux(&options.target, &objects, &output),
        "macos" => {
            eprintln!("macOS 目标请在其平台原生构建（本机可 --emit-object 生成 Mach-O 对象）");
            std::process::exit(2);
        }
        other => {
            eprintln!("暂不支持链接目标 `{other}`；可用 --emit-object 生成对象文件");
            std::process::exit(2);
        }
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

fn link_windows(target: &str, objects: &[PathBuf], output: &Path) {
    let sdk = locate_sdk().unwrap_or_else(|| {
        eprintln!("未找到工具链；请设置 SW_TOOLCHAIN 指向 llvm-mingw 目录");
        std::process::exit(2);
    });
    let (runtime_objects, need_compile) = match runtime_objects_for(&sdk) {
        Some(objects) => (objects, false),
        None => {
            let clang = sdk.mingw_clang.as_ref().expect("工具链缺少 clang");
            (compile_runtime_objects(clang, target, "windows"), true)
        }
    };
    let _ = need_compile;
    let lld = sdk.lld.as_path();
    let lib_dir = sdk.mingw_lib.as_path();
    let builtins = sdk.builtins.as_ref().expect("缺少 compiler-rt");
    let mut args: Vec<std::ffi::OsString> = vec!["-m".into(), "i386pep".into()];
    for object in objects {
        args.push(object.as_os_str().to_os_string());
    }
    for object in &runtime_objects {
        args.push(object.as_os_str().to_os_string());
    }
    args.push("-L".into());
    args.push(lib_dir.as_os_str().to_os_string());
    for library in ["-lucrt", "-lucrtbase", "-lkernel32"] {
        args.push(library.into());
    }
    args.push(builtins.as_os_str().to_os_string());
    args.push("-o".into());
    args.push(output.as_os_str().to_os_string());
    run_linker(lld, &args);
}

fn link_linux(target: &str, objects: &[PathBuf], output: &Path) {
    let sdk = locate_sdk().unwrap_or_else(|| {
        eprintln!("未找到工具链；请设置 SW_TOOLCHAIN 指向 llvm-mingw 目录");
        std::process::exit(2);
    });
    let clang = sdk
        .mingw_clang
        .as_ref()
        .expect("Linux 交叉链接需要工具链 clang 编译运行时");
    let runtime_objects = compile_runtime_objects(clang, target, "linux");
    let lld = sdk.lld.as_path();
    let musl_dir = musl_self_contained_dir().unwrap_or_else(|| {
        eprintln!("缺少 musl 目标库；请执行：rustup target add x86_64-unknown-linux-musl");
        std::process::exit(2);
    });
    let mut args: Vec<std::ffi::OsString> =
        vec!["-m".into(), "elf_x86_64".into(), "-static".into()];
    for object in objects {
        args.push(object.as_os_str().to_os_string());
    }
    for object in &runtime_objects {
        args.push(object.as_os_str().to_os_string());
    }
    for startup in ["crt1.o", "crti.o", "crtn.o"] {
        args.push(musl_dir.join(startup).as_os_str().to_os_string());
    }
    args.push(musl_dir.join("libc.a").as_os_str().to_os_string());
    args.push("-o".into());
    args.push(output.as_os_str().to_os_string());
    run_linker(lld, &args);
}

fn run_linker(lld: &Path, args: &[std::ffi::OsString]) {
    let status = Command::new(lld).args(args).status();
    if !status.map(|status| status.success()).unwrap_or(false) {
        eprintln!("链接失败（ld.lld）");
        std::process::exit(1);
    }
}

fn compile_runtime_objects(clang: &Path, target: &str, family: &str) -> Vec<PathBuf> {
    let cache_dir = PathBuf::from(".swcache").join("obj");
    let runtime_dir = env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("runtime");
    let runtime_c = runtime_dir.join("runtime.c");
    let runtime_s = runtime_dir.join("runtime.s");
    let suffix = if family == "windows" { "obj" } else { "o" };
    let target_tag = target.replace(|c: char| !c.is_ascii_alphanumeric(), "_");
    let runtime_obj = cache_dir.join(format!("runtime_{target_tag}.{suffix}"));
    let runtime_asm_obj = cache_dir.join(format!("runtime_asm_{target_tag}.{suffix}"));
    let startup_obj = cache_dir.join(format!("startup_{target_tag}.{suffix}"));
    let mut result = Vec::new();
    let mut tasks = vec![
        (
            runtime_c.clone(),
            runtime_obj.clone(),
            vec![
                "-target".to_owned(),
                target.to_owned(),
                "-O2".to_owned(),
                "-c".to_owned(),
            ],
        ),
        (
            runtime_s.clone(),
            runtime_asm_obj.clone(),
            vec!["-target".to_owned(), target.to_owned(), "-c".to_owned()],
        ),
    ];
    if family == "windows" {
        let startup_s = runtime_dir.join("startup.s");
        tasks.push((
            startup_s,
            startup_obj.clone(),
            vec!["-target".to_owned(), target.to_owned(), "-c".to_owned()],
        ));
    }
    for (source, object, prefix) in tasks {
        if compile_if_stale(clang, &source, &object, &prefix).is_err() {
            std::process::exit(1);
        }
        result.push(object.clone());
    }
    result
}

fn compile_if_stale(
    compiler: &Path,
    source: &Path,
    object: &Path,
    prefix: &[String],
) -> Result<(), ()> {
    let source_mtime = fs::metadata(source)
        .and_then(|metadata| metadata.modified())
        .ok();
    let object_mtime = fs::metadata(object)
        .and_then(|metadata| metadata.modified())
        .ok();
    let stale = !object.exists()
        || source_mtime
            .zip(object_mtime)
            .map(|(source, object)| source > object)
            .unwrap_or(true);
    if !stale {
        return Ok(());
    }
    let status = Command::new(compiler)
        .args(prefix)
        .arg(source)
        .arg("-o")
        .arg(object)
        .status();
    if !status.map(|status| status.success()).unwrap_or(false) {
        eprintln!("编译失败：{}", source.display());
        return Err(());
    }
    Ok(())
}

/// SDK 布局：<root>/swc.exe、<root>/bin/ld.lld.exe、<root>/lib/*.a 与预编译运行时、<root>/stdlib/
struct Sdk {
    lld: PathBuf,
    mingw_lib: PathBuf,
    builtins: Option<PathBuf>,
    mingw_clang: Option<PathBuf>,
    prebuilt_runtime: bool,
}

fn locate_sdk() -> Option<Sdk> {
    let exe_dir = env::current_exe().ok()?.parent()?.to_path_buf();
    let sdk_candidate = |root: PathBuf| -> Option<Sdk> {
        let lld = root.join("bin").join("ld.lld.exe");
        let mingw_lib = root.join("lib");
        if !lld.is_file() || !mingw_lib.is_dir() {
            return None;
        }
        let builtins = mingw_lib.join("libclang_rt.builtins-x86_64.a");
        let clang = root.join("bin").join("clang.exe");
        Some(Sdk {
            lld,
            mingw_lib,
            builtins: builtins.is_file().then_some(builtins),
            mingw_clang: clang.is_file().then_some(clang),
            prebuilt_runtime: runtime_objects_for_root(&root).is_some(),
        })
    };
    // 1) 可执行文件旁的 SDK 布局
    if let Some(sdk) = sdk_candidate(exe_dir.clone()) {
        return Some(sdk);
    }
    // 2) SW_TOOLCHAIN 指向的 llvm-mingw
    if let Some(path) = env::var_os("SW_TOOLCHAIN") {
        let root = PathBuf::from(path);
        if let Some(sdk) = sdk_candidate_for_mingw(root) {
            return Some(sdk);
        }
    }
    // 3) 默认 llvm-mingw 目录
    for candidate in [
        r"D:\llvm-mingw-20260616-ucrt-x86_64",
        r"C:\llvm-mingw-20260616-ucrt-x86_64",
        r"C:\llvm-mingw",
    ] {
        if let Some(sdk) = sdk_candidate_for_mingw(PathBuf::from(candidate)) {
            return Some(sdk);
        }
    }
    None
}

fn sdk_candidate_for_mingw(root: PathBuf) -> Option<Sdk> {
    let lld = root.join("bin").join("ld.lld.exe");
    let mingw_lib = root.join("x86_64-w64-mingw32").join("lib");
    if !lld.is_file() || !mingw_lib.is_dir() {
        return None;
    }
    let builtins = root
        .join("lib")
        .join("clang")
        .join("22")
        .join("lib")
        .join("windows")
        .join("libclang_rt.builtins-x86_64.a");
    let clang = root.join("bin").join("clang.exe");
    Some(Sdk {
        lld,
        mingw_lib,
        builtins: builtins.is_file().then_some(builtins),
        mingw_clang: clang.is_file().then_some(clang),
        prebuilt_runtime: false,
    })
}

fn runtime_objects_for_root(root: &Path) -> Option<Vec<PathBuf>> {
    let lib = root.join("lib");
    let runtime = lib.join("runtime.obj");
    let asm = lib.join("runtime_asm.obj");
    let startup = lib.join("startup.obj");
    (runtime.is_file() && asm.is_file() && startup.is_file()).then(|| vec![runtime, asm, startup])
}

fn runtime_objects_for(sdk: &Sdk) -> Option<Vec<PathBuf>> {
    if !sdk.prebuilt_runtime {
        return None;
    }
    runtime_objects_for_root(sdk.mingw_lib.parent()?)
}

fn musl_self_contained_dir() -> Option<PathBuf> {
    let home = env::var("RUSTUP_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(r"C:\Users\Administrator\.rustup"));
    let candidate = home
        .join("toolchains")
        .join("stable-x86_64-pc-windows-msvc")
        .join("lib")
        .join("rustlib")
        .join("x86_64-unknown-linux-musl")
        .join("lib")
        .join("self-contained");
    candidate.is_dir().then_some(candidate)
}

fn parse_options(args: &[String]) -> BuildOptions {
    let mut output = None;
    let mut target = None;
    let mut emit_object = None;
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "-o" | "--output" => {
                output = args.get(index + 1).map(PathBuf::from);
                index += 2;
            }
            "--target" => {
                target = args.get(index + 1).cloned();
                index += 2;
            }
            "--emit-object" => {
                emit_object = args.get(index + 1).map(PathBuf::from);
                index += 2;
            }
            _ => index += 1,
        }
    }
    BuildOptions {
        output,
        target: target.unwrap_or_else(default_target),
        emit_object,
    }
}

fn default_target() -> String {
    match env::consts::OS {
        "windows" => "x86_64-w64-windows-gnu".to_owned(),
        "linux" => "x86_64-unknown-linux-musl".to_owned(),
        "macos" => {
            if env::consts::ARCH == "aarch64" {
                "aarch64-apple-darwin".to_owned()
            } else {
                "x86_64-apple-darwin".to_owned()
            }
        }
        _ => "x86_64-unknown-linux-musl".to_owned(),
    }
}

fn target_family(target: &str) -> &str {
    if target.contains("windows") {
        "windows"
    } else if target.contains("linux") {
        "linux"
    } else if target.contains("darwin") {
        "macos"
    } else {
        "unknown"
    }
}

fn executable_suffix(target: &str) -> &str {
    if target_family(target) == "windows" {
        ".exe"
    } else {
        ""
    }
}
