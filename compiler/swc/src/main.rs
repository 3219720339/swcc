use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use sw_codegen_cranelift::compile_module_for_target;
use sw_common::{Diagnostics, Severity, Source};
use sw_frontend::Parser;
use sw_semantic::{MirModule, Type, analyze};

struct BuildOptions {
    output: Option<PathBuf>,
    target: String,
    emit_object: Option<PathBuf>,
    /// run 时透传给程序的命令行参数。
    run_args: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum BuildKind {
    #[default]
    Console,
    Dll,
    Lib,
}

#[derive(Default)]
struct SwConfig {
    kind: BuildKind,
    lib_name: String,
}

/// 极简 swcc.toml 解析：只读 [build] kind 与 [lib] name。
fn load_config(entry: &Path) -> SwConfig {
    let mut config = SwConfig::default();
    let mut dir = entry.parent().map(Path::to_path_buf).unwrap_or_default();
    loop {
        let candidate = dir.join("swcc.toml");
        if candidate.is_file() {
            if let Ok(text) = fs::read_to_string(&candidate) {
                let mut section = String::new();
                for line in text.lines() {
                    let line = line.trim();
                    if line.starts_with('[') && line.ends_with(']') {
                        section = line[1..line.len() - 1].trim().to_string();
                        continue;
                    }
                    if let Some((key, value)) = line.split_once('=') {
                        let key = key.trim();
                        let value = value.trim().trim_matches('"').trim_matches('\'');
                        match (section.as_str(), key) {
                            ("build", "kind") => {
                                config.kind = match value {
                                    "dll" => BuildKind::Dll,
                                    "lib" => BuildKind::Lib,
                                    _ => BuildKind::Console,
                                };
                            }
                            ("lib", "name") | ("project", "name") if config.lib_name.is_empty() => {
                                config.lib_name = value.to_string();
                            }
                            _ => {}
                        }
                    }
                }
            }
            break;
        }
        if !dir.pop() {
            break;
        }
    }
    config
}

fn main() {
    let started = Instant::now();
    let args: Vec<String> = env::args().collect();
    if args.len() == 2 && (args[1] == "--version" || args[1] == "-V") {
        println!("swc {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.len() == 2 && matches!(args[1].as_str(), "help" | "--help" | "-h") {
        print_help();
        return;
    }
    let command = args.get(1).map(String::as_str).unwrap_or("check");
    if command == "help" || command == "--help" || command == "-h" {
        print_help();
        return;
    }
    if !matches!(command, "check" | "build" | "run" | "test") {
        eprintln!("未知命令 `{command}`；可用 `swc help` 查看用法");
        std::process::exit(2);
    }
    let Some(path) = args.get(2) else {
        eprintln!(
            "用法: swc {command} <文件.sw> [-o 输出] [--target <triple>] [--emit-object <目标文件>]"
        );
        std::process::exit(2);
    };
    let options = parse_options(&args);
    if matches!(command, "run" | "test") && options.target != default_target() {
        eprintln!("`{command}` 只支持宿主目标；当前目标 {}", options.target);
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
        println!("用时：{} ms", started.elapsed().as_millis());
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

    let config = load_config(Path::new(path));
    let stem = Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "main".to_owned());
    let output = options.output.clone().unwrap_or_else(|| {
        let base = if config.lib_name.is_empty() {
            stem.clone()
        } else {
            config.lib_name.clone()
        };
        let suffix = match config.kind {
            BuildKind::Console => executable_suffix(&options.target).to_string(),
            BuildKind::Dll => match target_family(&options.target) {
                "windows" => ".dll".to_string(),
                "linux" => ".so".to_string(),
                "macos" => ".dylib".to_string(),
                _ => ".dll".to_string(),
            },
            BuildKind::Lib => match target_family(&options.target) {
                "windows" => ".lib".to_string(),
                _ => ".a".to_string(),
            },
        };
        PathBuf::from(format!("{base}{suffix}"))
    });

    match config.kind {
        BuildKind::Lib => build_lib(&options.target, &objects, &output, &result.modules),
        _ => {
            let dll = config.kind == BuildKind::Dll;
            match target_family(&options.target) {
                "windows" => link_windows(&options.target, &objects, &output, dll, &result.modules),
                "linux" => link_linux(&options.target, &objects, &output, dll),
                "macos" => link_macos(&options.target, &objects, &output, dll),
                other => {
                    eprintln!("暂不支持链接目标 `{other}`；可用 --emit-object 生成对象文件");
                    std::process::exit(2);
                }
            }
            // dll 附带头文件（声明导出函数，配合导入库直接链接调用）。
            if dll {
                write_lib_header(&result.modules, &output);
            }
        }
    }

    println!(
        "构建成功：{}（用时 {} ms）",
        output.display(),
        started.elapsed().as_millis()
    );
    if command == "run" || command == "test" {
        let run_path = fs::canonicalize(&output).unwrap_or(output.clone());
        let status = Command::new(&run_path).args(&options.run_args).status();
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

fn print_help() {
    println!("Sw 编译器 swc {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("用法: swc <命令> <文件.sw> [选项]");
    println!();
    println!("命令:");
    println!("  check              词法/语法/语义检查并生成 MIR");
    println!("  build              编译并链接生成可执行文件");
    println!("  run                编译、链接并运行");
    println!("  test               编译并运行 @test 测试（退出码=失败数）");
    println!("  help               显示本帮助");
    println!();
    println!("选项:");
    println!("  -o, --output <文件>     指定输出文件");
    println!("  --target <triple>       目标平台（如 x86_64-w64-windows-gnu、");
    println!("                          x86_64-unknown-linux-musl、aarch64-unknown-linux-musl）");
    println!("  --emit-object <文件>    只生成目标文件，不链接");
    println!();
    println!("环境变量:");
    println!("  SW_TOOLCHAIN  指向 llvm-mingw 工具链目录");
    println!("  SW_STDLIB     指向标准库目录（默认查找可执行文件旁或当前目录的 stdlib/）");
}

fn link_windows(
    target: &str,
    objects: &[PathBuf],
    output: &Path,
    dll: bool,
    modules: &[MirModule],
) {
    let sdk = locate_sdk(target).unwrap_or_else(|| {
        eprintln!("未找到工具链；请设置 SW_TOOLCHAIN 指向 llvm-mingw 目录");
        std::process::exit(2);
    });
    let (runtime_objects, need_compile) = if dll {
        let clang = sdk.mingw_clang.as_ref().expect("工具链缺少 clang");
        (
            compile_runtime_objects(clang, target, "windows", true),
            true,
        )
    } else {
        match runtime_objects_for(&sdk, target) {
            Some(objects) => (objects, false),
            None => {
                let clang = sdk.mingw_clang.as_ref().expect("工具链缺少 clang");
                (
                    compile_runtime_objects(clang, target, "windows", false),
                    true,
                )
            }
        }
    };
    let _ = need_compile;
    let lld = sdk.lld.as_path();
    let lib_dir = sdk.mingw_lib.as_path();
    let builtins = sdk.builtins.as_ref().expect("缺少 compiler-rt");
    let mut args: Vec<std::ffi::OsString> = vec!["-m".into(), pe_emulation(arch_of(target)).into()];
    if dll {
        args.push("--dll".into());
        let implib = output.with_extension("lib");
        args.push("--out-implib".into());
        args.push(implib.as_os_str().to_os_string());
        // .def：自动导出所有 `export` 标记的顶层函数（用户函数名 = stable 符号）。
        let def_path = PathBuf::from(".swcache").join("obj").join(
            output
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "sw.def".to_string())
                .replace(".dll", ".def"),
        );
        let mut def = String::from("EXPORTS\n");
        for module in modules {
            for function in &module.functions {
                if function.extern_c
                    || function.user_name.is_empty()
                    || function.name == "sw_user_main"
                    || !function.exported
                {
                    continue;
                }
                def.push_str(&format!("{} = {}\n", function.user_name, function.name));
            }
        }
        if let Err(error) = fs::write(&def_path, def) {
            eprintln!("无法写出 .def 文件 {}：{error}", def_path.display());
            std::process::exit(2);
        }
        args.push(def_path.as_os_str().to_os_string());
    } else {
        args.push("--gc-sections".into());
    }
    for object in objects {
        args.push(object.as_os_str().to_os_string());
    }
    for object in &runtime_objects {
        args.push(object.as_os_str().to_os_string());
    }
    args.push("-L".into());
    args.push(lib_dir.as_os_str().to_os_string());
    for library in [
        "-lucrt",
        "-lucrtbase",
        "-lkernel32",
        "-lshell32",
        "-lole32",
        "-lws2_32",
    ] {
        args.push(library.into());
    }
    args.push(builtins.as_os_str().to_os_string());
    args.push("-o".into());
    args.push(output.as_os_str().to_os_string());
    run_linker(lld, &args);
}

fn link_linux(target: &str, objects: &[PathBuf], output: &Path, dll: bool) {
    let sdk = locate_sdk(target).unwrap_or_else(|| {
        eprintln!("未找到工具链；请设置 SW_TOOLCHAIN 指向 llvm-mingw 目录");
        std::process::exit(2);
    });
    let (runtime_objects, need_compile) = if dll {
        let clang = sdk.mingw_clang.as_ref().unwrap_or_else(|| {
            eprintln!("缺少 clang 且无预编译运行时，无法链接 Linux 目标");
            std::process::exit(2);
        });
        (compile_runtime_objects(clang, target, "linux", true), true)
    } else {
        match runtime_objects_for(&sdk, target) {
            Some(objects) => (objects, false),
            None => {
                let clang = sdk.mingw_clang.as_ref().unwrap_or_else(|| {
                    eprintln!("缺少 clang 且无预编译运行时，无法链接 Linux 目标");
                    std::process::exit(2);
                });
                (compile_runtime_objects(clang, target, "linux", false), true)
            }
        }
    };
    let _ = need_compile;
    let lld = sdk.lld.as_path();
    let musl_dir = musl_self_contained_dir(target).unwrap_or_else(|| {
        eprintln!(
            "缺少 musl 目标库；请执行：rustup target add {}",
            musl_target(target)
        );
        std::process::exit(2);
    });
    let mut args: Vec<std::ffi::OsString> =
        vec!["-m".into(), elf_emulation(arch_of(target)).into()];
    if dll {
        args.push("-shared".into());
    } else {
        args.push("-static".into());
        args.push("--gc-sections".into());
    }
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
    if arch_of(target) == "aarch64" {
        // musl 的 128 位软浮点路径需要 compiler-rt（x86_64 用不到，aarch64 必需）。
        let builtins = compiler_builtins_rlib(&musl_dir).unwrap_or_else(|| {
            eprintln!("缺少 compiler-builtins（aarch64 musl）");
            std::process::exit(2);
        });
        args.push(builtins.as_os_str().to_os_string());
    }
    args.push("-o".into());
    args.push(output.as_os_str().to_os_string());
    run_linker(lld, &args);
}

/// macOS 原生链接：用系统 clang（cc）编译运行时并链接，目标机直接出可执行文件。
fn link_macos(target: &str, objects: &[PathBuf], output: &Path, dll: bool) {
    let cc = Path::new("cc");
    let runtime_objects = compile_runtime_objects(cc, target, "macos", dll);
    let mut args: Vec<std::ffi::OsString> = vec!["-target".into(), target.into()];
    if dll {
        args.push("-dynamiclib".into());
    } else {
        args.push("-Wl,-dead_strip".into());
    }
    for object in objects {
        args.push(object.as_os_str().to_os_string());
    }
    for object in &runtime_objects {
        args.push(object.as_os_str().to_os_string());
    }
    args.push("-o".into());
    args.push(output.as_os_str().to_os_string());
    run_cc(cc, &args);
}

fn run_linker(lld: &Path, args: &[std::ffi::OsString]) {
    let status = Command::new(lld).args(args).status();
    if !status.map(|status| status.success()).unwrap_or(false) {
        eprintln!("链接失败（ld.lld）");
        std::process::exit(1);
    }
}

fn run_cc(cc: &Path, args: &[std::ffi::OsString]) {
    let status = Command::new(cc).args(args).status();
    if !status.map(|status| status.success()).unwrap_or(false) {
        eprintln!("链接失败（cc）");
        std::process::exit(1);
    }
}

/// 静态库：用 llvm-ar 打包 Sw 模块对象（不含运行时，使用方自行链接运行时）。
fn build_lib(target: &str, objects: &[PathBuf], output: &Path, modules: &[MirModule]) {
    let sdk = locate_sdk(target).unwrap_or_else(|| {
        eprintln!("未找到工具链；请设置 SW_TOOLCHAIN 指向 llvm-mingw 目录");
        std::process::exit(2);
    });
    let bin_dir = sdk.lld.parent().expect("lld 所在目录");
    let ar = bin_dir.join(host_exe_name("llvm-ar"));
    if !ar.is_file() {
        eprintln!("缺少 llvm-ar，无法打包静态库；可先用 --emit-object 生成对象");
        std::process::exit(2);
    }
    let mut args: Vec<std::ffi::OsString> = vec!["rcs".into(), output.as_os_str().to_os_string()];
    for object in objects {
        args.push(object.as_os_str().to_os_string());
    }
    // 用户函数名转发 stub（头文件按用户函数名直接调用）。
    if let Some(stub) = build_lib_stub(target, modules) {
        args.push(stub.as_os_str().to_os_string());
    }
    let status = Command::new(&ar).args(&args).status();
    if !status.map(|status| status.success()).unwrap_or(false) {
        eprintln!("打包静态库失败（llvm-ar）");
        std::process::exit(1);
    }
    write_lib_header(modules, output);
}

/// 生成转发 stub：`用户函数名(params) { return stable名(params); }`，
/// 编译成对象并入静态库，使 C 头文件按用户函数名直接链接调用。
fn build_lib_stub(target: &str, modules: &[MirModule]) -> Option<PathBuf> {
    let sdk = locate_sdk(target)?;
    let clang = if target_family(target) == "macos" {
        PathBuf::from("cc")
    } else {
        sdk.mingw_clang.clone()?
    };
    let mut c = String::new();
    c.push_str(
        "typedef struct sw_string { char* data; long long len; } sw_string;\n\
         typedef struct sw_array { long long len; long long cap; void* data; } sw_array;\n",
    );
    let mut has_export = false;
    for module in modules {
        for function in &module.functions {
            if function.extern_c
                || function.user_name.is_empty()
                || function.name == "sw_user_main"
                || !function.exported
            {
                continue;
            }
            has_export = true;
            let ret = c_type(&function.ret);
            let params: Vec<(String, String)> = function
                .params
                .iter()
                .enumerate()
                .map(|(index, param)| (c_type(&param.ty), format!("p{index}")))
                .collect();
            let signature = params
                .iter()
                .map(|(ty, name)| format!("{ty} {name}"))
                .collect::<Vec<_>>()
                .join(", ");
            let call_args = params
                .iter()
                .map(|(_, name)| name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            c.push_str(&format!(
                "{ret} {}({signature});\n\
                 {ret} {}({signature}) {{\n    return {}({call_args});\n}}\n",
                function.name, function.user_name, function.name
            ));
        }
    }
    if !has_export {
        return None;
    }
    let cache_dir = PathBuf::from(".swcache").join("obj");
    if let Err(error) = fs::create_dir_all(&cache_dir) {
        eprintln!("无法创建缓存目录：{error}");
        std::process::exit(2);
    }
    let stub_c = cache_dir.join("lib_stub.c");
    if let Err(error) = fs::write(&stub_c, c) {
        eprintln!("无法写出 stub：{error}");
        std::process::exit(2);
    }
    let target_tag = target.replace(|c: char| !c.is_ascii_alphanumeric(), "_");
    let suffix = if target_family(target) == "windows" {
        "obj"
    } else {
        "o"
    };
    let stub_obj = cache_dir.join(format!("lib_stub_{target_tag}.{suffix}"));
    let status = Command::new(&clang)
        .args([
            "-target",
            target,
            "-O2",
            "-ffunction-sections",
            "-fdata-sections",
            "-c",
        ])
        .arg(&stub_c)
        .arg("-o")
        .arg(&stub_obj)
        .status();
    if !status.map(|status| status.success()).unwrap_or(false) {
        eprintln!("编译转发 stub 失败（{}）", clang.display());
        std::process::exit(1);
    }
    Some(stub_obj)
}

/// Sw 类型 → C 声明类型。
fn c_type(ty: &Type) -> String {
    match ty.without_nullable() {
        Type::Void => "void".to_string(),
        Type::Str => "sw_string*".to_string(),
        Type::Array(_) => "sw_array*".to_string(),
        Type::Class(_) | Type::Struct(_) => "void*".to_string(),
        Type::F32 | Type::F64 => "double".to_string(),
        Type::Bool => "long long".to_string(),
        Type::Char => "long long".to_string(),
        _ => "long long".to_string(),
    }
}

/// 生成配套 C 头文件（swmath.lib → swmath.h），声明全部导出函数的 C 签名。
fn write_lib_header(modules: &[MirModule], output: &Path) {
    let header_path = output.with_extension("h");
    let stem = output
        .file_stem()
        .map(|s| {
            s.to_string_lossy()
                .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        })
        .unwrap_or_else(|| "sw".to_string());
    let guard = format!("SW_{stem}_H");
    let mut text = String::new();
    text.push_str(&format!("#ifndef {guard}\n#define {guard}\n\n"));
    text.push_str(
        "// 由 swc 生成：Sw 模块导出的 C 接口（按源码 `export function` 收集）。\n\
         typedef struct sw_string { char* data; long long len; } sw_string;\n\
         typedef struct sw_array { long long len; long long cap; void* data; } sw_array;\n\n",
    );
    for module in modules {
        for function in &module.functions {
            if function.extern_c
                || function.user_name.is_empty()
                || function.name == "sw_user_main"
                || !function.exported
            {
                continue;
            }
            let ret = c_type(&function.ret);
            let params: Vec<String> = function
                .params
                .iter()
                .map(|param| format!("{} {}", c_type(&param.ty), param.name))
                .collect();
            text.push_str(&format!(
                "extern {} {}({});\n",
                ret,
                function.user_name,
                params.join(", "),
            ));
        }
    }
    text.push_str(&format!("\n#endif // {guard}\n"));
    if let Err(error) = fs::write(&header_path, text) {
        eprintln!("无法写出头文件 {}：{error}", header_path.display());
        std::process::exit(2);
    }
    println!("头文件已生成：{}", header_path.display());
}

fn compile_runtime_objects(
    clang: &Path,
    target: &str,
    family: &str,
    no_main: bool,
) -> Vec<PathBuf> {
    let cache_dir = PathBuf::from(".swcache").join("obj");
    let runtime_dir = locate_runtime_dir().unwrap_or_else(|| {
        eprintln!("找不到 runtime 源目录；请设置 SW_RUNTIME 指向 swcc/runtime 目录");
        std::process::exit(2);
    });
    let runtime_c = runtime_dir.join("runtime.c");
    let runtime_s = runtime_dir.join(runtime_asm_file(arch_of(target)));
    let suffix = if family == "windows" { "obj" } else { "o" };
    let target_tag = target.replace(|c: char| !c.is_ascii_alphanumeric(), "_");
    let runtime_obj = cache_dir.join(format!(
        "runtime_{}{target_tag}.{suffix}",
        if no_main { "dll_" } else { "" }
    ));
    let runtime_asm_obj = cache_dir.join(format!("runtime_asm_{target_tag}.{suffix}"));
    let startup_obj = cache_dir.join(format!("startup_{target_tag}.{suffix}"));
    let mut result = Vec::new();
    let mut main_args = vec!["-target".to_owned(), target.to_owned(), "-O2".to_owned()];
    if no_main {
        main_args.push("-DSW_NO_MAIN".to_owned());
    }
    main_args.push("-ffunction-sections".to_owned());
    main_args.push("-fdata-sections".to_owned());
    main_args.push("-c".to_owned());
    let mut tasks = vec![
        (runtime_c.clone(), runtime_obj.clone(), main_args),
        (
            runtime_s.clone(),
            runtime_asm_obj.clone(),
            vec!["-target".to_owned(), target.to_owned(), "-c".to_owned()],
        ),
    ];
    if family == "windows" && !no_main {
        let startup_s = runtime_dir.join(startup_file(arch_of(target)));
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

/// 定位 runtime 源目录：优先 SW_RUNTIME，其次当前目录/可执行文件附近的 runtime/。
fn locate_runtime_dir() -> Option<PathBuf> {
    if let Some(path) = env::var_os("SW_RUNTIME") {
        let candidate = PathBuf::from(path);
        if candidate.join("runtime.c").is_file() {
            return Some(candidate);
        }
    }
    let cwd = env::current_dir().ok()?;
    if cwd.join("runtime").join("runtime.c").is_file() {
        return Some(cwd.join("runtime"));
    }
    let mut anchor = env::current_exe().ok()?.parent()?.to_path_buf();
    for _ in 0..6 {
        let candidate = anchor.join("runtime");
        if candidate.join("runtime.c").is_file() {
            return Some(candidate);
        }
        anchor = anchor.parent()?.to_path_buf();
    }
    None
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
    let mut sidecar = object.as_os_str().to_owned();
    sidecar.push(".hash");
    let sidecar = PathBuf::from(sidecar);
    if object.exists() {
        let mtime_stale = source_mtime
            .zip(object_mtime)
            .map(|(source, object)| source > object)
            .unwrap_or(true);
        // 侧车文件记录「源文件内容哈希 + 编译参数」。
        // 内容与参数都一致才复用：mtime 旧但内容没变（git 检出只改时间戳）
        // 时不重编；编译参数变更（如 -ffunction-sections）也会触发重编。
        let current = file_hash(source)
            .map(|hash| format!("{hash}\n{}", prefix.join(" ")))
            .unwrap_or_default();
        let same = !current.is_empty()
            && fs::read_to_string(&sidecar)
                .map(|saved| saved == current)
                .unwrap_or(false);
        // 旧对象没有 sidecar（升级前编译）时按 mtime 判断。
        if same || !mtime_stale && current.is_empty() {
            return Ok(());
        }
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
    if let Some(hash) = file_hash(source) {
        let _ = fs::write(&sidecar, format!("{hash}\n{}", prefix.join(" ")));
    }
    Ok(())
}

/// FNV-1a 64 位内容哈希（十六进制），用于运行时缓存失效判断。
fn file_hash(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in &bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Some(format!("{hash:016x}"))
}

/// SDK 布局：<root>/swc.exe、<root>/bin/ld.lld.exe、<root>/lib/*.a 与预编译运行时、<root>/stdlib/
struct Sdk {
    lld: PathBuf,
    mingw_lib: PathBuf,
    builtins: Option<PathBuf>,
    mingw_clang: Option<PathBuf>,
    prebuilt_runtime: bool,
}

fn locate_sdk(target: &str) -> Option<Sdk> {
    let exe_dir = env::current_exe().ok()?.parent()?.to_path_buf();
    // 1) 可执行文件旁的 SDK 布局
    if let Some(sdk) = sdk_candidate_for_layout(exe_dir, target) {
        return Some(sdk);
    }
    // 2) SW_TOOLCHAIN 指向的 llvm-mingw
    if let Some(path) = env::var_os("SW_TOOLCHAIN") {
        let root = PathBuf::from(path);
        if let Some(sdk) = sdk_candidate_for_mingw(root, target) {
            return Some(sdk);
        }
    }
    // 3) 默认 llvm-mingw 目录
    for candidate in [
        r"D:\llvm-mingw-20260616-ucrt-x86_64",
        r"C:\llvm-mingw-20260616-ucrt-x86_64",
        r"C:\llvm-mingw",
    ] {
        if let Some(sdk) = sdk_candidate_for_mingw(PathBuf::from(candidate), target) {
            return Some(sdk);
        }
    }
    None
}

fn sdk_candidate_for_layout(root: PathBuf, target: &str) -> Option<Sdk> {
    let arch = arch_of(target);
    let lld = root.join("bin").join(host_exe_name("ld.lld"));
    let mingw_lib = root.join("lib");
    if !lld.is_file() || !mingw_lib.is_dir() {
        return None;
    }
    let builtins = mingw_lib.join(builtins_stem(arch));
    let clang = root.join("bin").join(host_exe_name("clang"));
    Some(Sdk {
        lld,
        mingw_lib,
        builtins: builtins.is_file().then_some(builtins),
        mingw_clang: clang.is_file().then_some(clang),
        prebuilt_runtime: runtime_objects_for_root(&root, target).is_some(),
    })
}

fn sdk_candidate_for_mingw(root: PathBuf, target: &str) -> Option<Sdk> {
    let arch = arch_of(target);
    let lld = root.join("bin").join(host_exe_name("ld.lld"));
    let mingw_lib = root.join(mingw_arch_dir(arch)).join("lib");
    if !lld.is_file() || !mingw_lib.is_dir() {
        return None;
    }
    let builtins = root
        .join("lib")
        .join("clang")
        .join("22")
        .join("lib")
        .join("windows")
        .join(builtins_stem(arch));
    let clang = root.join("bin").join(host_exe_name("clang"));
    Some(Sdk {
        lld,
        mingw_lib,
        builtins: builtins.is_file().then_some(builtins),
        mingw_clang: clang.is_file().then_some(clang),
        prebuilt_runtime: false,
    })
}

fn runtime_objects_for_root(root: &Path, target: &str) -> Option<Vec<PathBuf>> {
    let lib = root.join("lib");
    let arch = arch_of(target);
    match target_family(target) {
        "windows" => {
            let (runtime, asm, startup) = if arch == "aarch64" {
                (
                    lib.join("runtime_aarch64.obj"),
                    lib.join("runtime_asm_aarch64.obj"),
                    lib.join("startup_aarch64.obj"),
                )
            } else {
                (
                    lib.join("runtime.obj"),
                    lib.join("runtime_asm.obj"),
                    lib.join("startup.obj"),
                )
            };
            (runtime.is_file() && asm.is_file() && startup.is_file())
                .then(|| vec![runtime, asm, startup])
        }
        "linux" => {
            let runtime = lib.join(format!("runtime_{arch}.o"));
            let asm = lib.join(format!("runtime_asm_{arch}.o"));
            (runtime.is_file() && asm.is_file()).then(|| vec![runtime, asm])
        }
        _ => None,
    }
}

fn runtime_objects_for(sdk: &Sdk, target: &str) -> Option<Vec<PathBuf>> {
    if !sdk.prebuilt_runtime {
        return None;
    }
    runtime_objects_for_root(sdk.mingw_lib.parent()?, target)
}

fn musl_self_contained_dir(target: &str) -> Option<PathBuf> {
    let triple = if target.contains("musl") {
        target
    } else {
        musl_target(target)
    };
    // 1) SDK 自带 musl 库（<root>/musl/<triple>，解压即用）
    if let Some(exe_dir) = env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        let sdk_candidate = exe_dir.join("musl").join(triple);
        if sdk_candidate.is_dir() {
            return Some(sdk_candidate);
        }
    }
    // 2) rustup 目标库
    let candidate = rustup_toolchain_dir()?
        .join("lib")
        .join("rustlib")
        .join(triple)
        .join("lib")
        .join("self-contained");
    candidate.is_dir().then_some(candidate)
}

/// rustup 的 <triple>/lib 下 compiler-builtins rlib（ELF 归档），
/// aarch64 Linux 静态链接时给 lld 补齐软浮点辅助函数。
fn compiler_builtins_rlib(musl_dir: &Path) -> Option<PathBuf> {
    for dir in [musl_dir, musl_dir.parent()?] {
        let found = fs::read_dir(dir)
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .map(|name| {
                        let name = name.to_string_lossy();
                        name.starts_with("libcompiler_builtins-") && name.ends_with(".rlib")
                    })
                    .unwrap_or(false)
            });
        if found.is_some() {
            return found;
        }
    }
    None
}

fn rustup_toolchain_dir() -> Option<PathBuf> {
    let home = env::var("RUSTUP_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            if env::consts::OS == "windows" {
                PathBuf::from(r"C:\Users\Administrator\.rustup")
            } else {
                PathBuf::from(env::var("HOME").unwrap_or_default()).join(".rustup")
            }
        });
    let mut entries = fs::read_dir(home.join("toolchains"))
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path());
    let mut chosen = entries.next()?;
    if !chosen.to_string_lossy().contains("stable") {
        for path in entries {
            if path.to_string_lossy().contains("stable") {
                chosen = path;
                break;
            }
        }
    }
    Some(chosen)
}

fn musl_target(target: &str) -> &str {
    if arch_of(target) == "aarch64" {
        "aarch64-unknown-linux-musl"
    } else {
        "x86_64-unknown-linux-musl"
    }
}

fn parse_options(args: &[String]) -> BuildOptions {
    let mut output = None;
    let mut target = None;
    let mut emit_object = None;
    let mut run_args = Vec::new();
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
            flag if flag.starts_with('-') => index += 1,
            value => {
                run_args.push(value.to_owned());
                index += 1;
            }
        }
    }
    BuildOptions {
        output,
        target: target.unwrap_or_else(default_target),
        emit_object,
        run_args,
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

fn arch_of(target: &str) -> &str {
    if target.contains("aarch64") {
        "aarch64"
    } else {
        "x86_64"
    }
}

fn pe_emulation(arch: &str) -> &'static str {
    match arch {
        "aarch64" => "aarch64pe",
        _ => "i386pep",
    }
}

fn elf_emulation(arch: &str) -> &'static str {
    match arch {
        "aarch64" => "aarch64elf",
        _ => "elf_x86_64",
    }
}

fn mingw_arch_dir(arch: &str) -> &'static str {
    match arch {
        "aarch64" => "aarch64-w64-mingw32",
        _ => "x86_64-w64-mingw32",
    }
}

fn builtins_stem(arch: &str) -> String {
    format!("libclang_rt.builtins-{arch}.a")
}

fn host_exe_name(basename: &str) -> String {
    if env::consts::OS == "windows" {
        format!("{basename}.exe")
    } else {
        basename.to_owned()
    }
}

fn runtime_asm_file(arch: &str) -> &'static str {
    match arch {
        "aarch64" => "runtime_aarch64.s",
        _ => "runtime_x64.S",
    }
}

fn startup_file(arch: &str) -> &'static str {
    match arch {
        "aarch64" => "startup_aarch64.s",
        _ => "startup.s",
    }
}

fn executable_suffix(target: &str) -> &str {
    if target_family(target) == "windows" {
        ".exe"
    } else {
        ""
    }
}
