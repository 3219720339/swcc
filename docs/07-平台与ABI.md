# 07 平台与 ABI

状态：规范基线

## 1. 目标平台

| 目标 | 架构 | 对象格式 | 链接器 |
|---|---|---|---|
| `x86_64-pc-windows-gnu` | x64 | COFF | lld + MinGW-w64 CRT |
| `aarch64-pc-windows-gnu` | arm64 | COFF | lld + MinGW-w64 CRT |
| `x86_64-unknown-linux-gnu` | x64 | ELF | lld + 系统 libc |
| `aarch64-unknown-linux-gnu` | arm64 | ELF | lld + 系统 libc |
| `x86_64-apple-darwin` | x64 | Mach-O | lld + 系统 libSystem |
| `aarch64-apple-darwin` | arm64 | Mach-O | lld + 系统 libSystem |

- v0.1 支持编译为当前主机目标；交叉目标对象（`--emit-object`）支持 ELF/COFF/Mach-O 任意组合生成。
- macOS 目标建议在 macOS 上原生链接（本机用系统 `cc` 编译运行时并链接；交叉链接无签名，仅限开发）。
- Windows 使用随 SDK 分发的 MinGW-w64 运行库，不依赖 Visual Studio；UCRT 由 Windows 10+ 系统提供。

## 2. 数据布局

- 指针宽度 = 目标架构字宽（64 位目标 8 字节）。
- 整数宽度即类型宽度；`char` 4 字节；`bool` 1 字节。
- 结构体按目标 ABI 对齐：成员按其最大对齐排布，结构体大小向上取整到对齐。
- `string` 运行时布局为 `{ ptr, len }`；`T[]` 为 `{ ptr, len, capacity }`（运行时 ABI 为准）。
- 数组元素连续存放，元素大小 = 目标 ABI 布局大小。

## 3. 调用约定

- Sw 内部函数：平台默认 C ABI 兼容约定（Windows x64 / SysV / arm64 AAPCS），便于与运行时间接。
- `extern c function`：严格 C ABI，参数与返回值按 C 规则映射。
- 虚方法：通过 vtable 指针 + 索引派发；接口引用布局在 03 类型系统中定义。

## 4. C ABI 映射

| Sw | C |
|---|---|
| `bool` | `bool`（1 字节） |
| `i8..i64/u8..u64` | `int8_t..` 对应 |
| `f32/f64` | `float/double` |
| `char` | `uint32_t` |
| `ptr<T>` | `T*` |
| `string` | 运行时结构 `{ const char* data; size_t len; }` |
| `T[]` | 运行时结构 `{ T* data; size_t len; size_t capacity; }` |
| `extern c function` 函数指针 | C 函数指针 |

规则：

- `extern c` 参数与返回值必须是 C ABI 可表示类型（数值、指针、extern 类型）；`string`/`T[]` 以运行时结构体传值。
- 异常不得穿过 C 边界。
- C 回调：Sw 函数可安全转换为 `extern c` 函数指针，运行时负责桥接与生命周期。

## 5. 链接与运行时

- 可执行程序默认静态链接 Sw 运行时（`sw_runtime.a`）。
- 链接输入：用户代码目标文件 + 运行时静态库 + 目标平台系统库（Windows: kernel32 等；Linux: libc；macOS: libSystem）。
- 交叉链接使用 lld，不需要目标平台自带链接器。
- 运行时内存由保守式 GC 管理（见 04 内存模型），平台差异（栈顶/数据段定位）在 runtime.c 内适配。
- 调试构建输出调试信息（v0.2 完善前先保证正确性）；Release 输出默认 `-O2` 等价优化。

## 6. 分发约束

- 运行时源码为 C（`runtime/runtime.c`），按目标平台编译为对象/静态库后随 SDK 打包。
- SDK 内不含任何需要用户安装的第三方工具链。
- 将来新增目标（如 riscv64）只需在 CI 增加对应运行时构建，编译器代码无需改动（Cranelift 已支持）。
