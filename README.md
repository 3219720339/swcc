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

## 下载与发布（v0.1.2）

三平台解压即用 SDK 由 GitHub Actions 在 `v*` 标签上自动构建并挂到 Release：

- Windows：`swc-windows-x64-0.1.2.zip`（swc + lld + MinGW UCRT 库 + 预编译运行时，解压即用，无需任何外部工具链；用法见 [docs/09-发布与分发.md](docs/09-发布与分发.md)）
- Linux：`swc-linux-x64-0.1.2.tar.gz`（swc + lld + musl 静态库 + 预编译运行时，无需安装任何工具链）
- macOS：`swc-macos-0.1.2.zip`（原生链接使用系统 `cc`，仅需 swc + 标准库）

每个 Release 同时附 `SHA256SUMS`（GNU 风格校验和清单，由 CI 对产物计算生成），下载后可校验完整性：

```bash
sha256sum -c SHA256SUMS    # 在包含各 SDK 归档的目录下执行
```

仓库当前托管在 Gitee；要让 CI/Release 在 GitHub 上运行，需要把仓库镜像过去：

```bash
# 在 GitHub 建一个空仓库（如 swcc），然后：
git remote add github https://github.com/<你的用户名>/swcc.git
git push github main --tags
```

之后每次打 `v0.1.2` 之类的标签并推送，GitHub 会自动跑测试、打包三平台 SDK、生成校验和并创建 Release。

## 构建与验证

- `cargo test --workspace`：词法/语法/AST、语义/MIR、Cranelift 代码生成测试（含 ELF/Mach-O 与 aarch64 对象格式）
- Linux 目标先安装：`rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl`
- 链接工具链通过环境变量 `SW_TOOLCHAIN` 指向 llvm-mingw 目录；Windows 上也支持可执行文件旁的 SDK 布局
- [.github/workflows/ci.yml](.github/workflows/ci.yml)：Windows / Ubuntu / macOS 三平台矩阵（仓库在 Gitee，需镜像到 GitHub 才会触发）
  - Windows：原生构建运行 + 打包解压即用 SDK（上传 artifact）
  - Linux：x86_64 musl 原生运行 + aarch64 musl 静态链接验证（ELF 机器类型校验）
  - macOS：系统 `cc` 原生链接运行；Apple Silicon runner 上即 aarch64 端到端验证

已知限制：Windows aarch64（COFF）暂不支持——Cranelift 对象写入器尚未实现 AArch64 COFF 重定位，跨平台对象验证走 ELF/Mach-O。
