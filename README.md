# Sw 编译器（swcc）

Sw 是一门静态强类型、面向本机程序的编译型语言，表层语法接近 JavaScript/TypeScript。

目标：

- 跨平台：同一份源码可在 Windows / Linux / macOS（x64 / arm64）编译运行
- 自包含工具链：编译器单包分发，不依赖用户安装 Clang / LLVM / Visual Studio
- JS 风格语法：`let/const`、`function`、`class`、箭头函数、模板字符串、`import/export`

技术路线：纯 Rust 前端（词法/语法/AST）+ 语义层（名称解析/类型检查/MIR）+ Cranelift 进程内代码生成 + lld 链接 + Rust 预编译运行时。

语言规范见 [docs/README.md](docs/README.md)。
