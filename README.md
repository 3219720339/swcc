# Sw 编译器（swcc）

Sw 是一门静态强类型、面向本机程序的编译型语言，表层语法接近 JavaScript/TypeScript。

目标：

- 跨平台：同一份源码可在 Windows / Linux / macOS（x64 / arm64）编译运行
- 自包含工具链：编译器单包分发，不依赖用户安装 Clang / LLVM / Visual Studio
- JS 风格语法：`let/const`、`function`、`class`、箭头函数、模板字符串、`import/export`

技术路线：纯 Rust 前端（词法/语法/AST）+ 语义层（名称解析/类型检查/MIR）+ Cranelift 进程内代码生成 + lld 链接 + Rust 预编译运行时。

语言规范见 [docs/README.md](docs/README.md)。

## 已实现能力

- 语义/后端：struct 值语义、`**` 幂、`++/--` 与赋值表达式、浮点全链路、泛型函数单态化
- 内存管理：运行时保守式标记-清除 GC（栈 + 数据段 + 堆内引用扫描）
- 标准库：`std/io`（print/println/read_line）、`std/math`、`std/fs`（文件读写）、`std/string`（查找/子串）
- CLI：`swc help`、`--version`、构建耗时输出、`--target` 交叉编译、`--emit-object`

## 构建与验证

- `cargo test --workspace`：词法/语法/AST、语义/MIR、Cranelift 代码生成测试（含 ELF/Mach-O 与 aarch64 对象格式）
- Linux 目标先安装：`rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl`
- 链接工具链通过环境变量 `SW_TOOLCHAIN` 指向 llvm-mingw 目录；Windows 上也支持可执行文件旁的 SDK 布局
- [.github/workflows/ci.yml](.github/workflows/ci.yml)：Windows / Ubuntu / macOS 三平台矩阵（仓库在 Gitee，需镜像到 GitHub 才会触发）
  - Windows：原生构建运行 + 打包解压即用 SDK（上传 artifact）
  - Linux：x86_64 musl 原生运行 + aarch64 musl 静态链接验证（ELF 机器类型校验）
  - macOS：系统 `cc` 原生链接运行；Apple Silicon runner 上即 aarch64 端到端验证

已知限制：Windows aarch64（COFF）暂不支持——Cranelift 对象写入器尚未实现 AArch64 COFF 重定位，跨平台对象验证走 ELF/Mach-O。
