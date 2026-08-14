# 构建产物演示（swcc.toml）

三个子项目演示 `swcc.toml` 的 `[build] kind` 三种产物：

| 目录 | kind | 产物 |
| --- | --- | --- |
| `math-lib/` | `lib` | 静态库 swmath.lib / swmath.a |
| `greet-dll/` | `dll` | swgreet.dll + 导入库（Linux .so / macOS .dylib） |
| `hello-app/` | `console` | hello-app.exe（可执行程序，默认） |

## 用法

```bash
# 1. 静态库：math-lib
cd examples/build-demo/math-lib
swc build math.sw          # 产出 swmath.lib（Windows）/ swmath.a

# 2. DLL：greet-dll
cd examples/build-demo/greet-dll
swc build greeter.sw       # 产出 swgreet.dll + swgreet.lib（Linux .so / macOS .dylib）

# 3. 控制台程序：hello-app
cd examples/build-demo/hello-app
swc build main.sw          # 产出 hello-app.exe
swc run main.sw            # 直接编译运行
```

> 说明：`swcc.toml` 会在入口文件所在目录向上查找；导出符号是固定名
> `sw_fn_<模块文件名>_<函数名>`（重载函数追加参数类型缩写，如
> `sw_fn_greeter_greet_s` = greet(string)）。
> **导出集合以 .sw 的 `export` 标记为准**：顶层 `export function` 自动成为
> dll 导出（Windows .def 别名用用户函数名，外部按 `greet`/`double` 调用）
> 与 lib 头文件声明；toml 不需要写函数名，.def 自动生成到 .swcache 缓存。

## DLL 动态加载（不需要 .lib）

dll 本身支持运行时动态加载（LoadLibrary/dlopen + GetProcAddress/dlsym），
不需要链接期导入库。见 `greet-dll/dynamic-load.c`：

```bash
cd examples/build-demo/greet-dll
swc build greeter.sw            # 生成 swgreet.dll
clang dynamic-load.c -o dynamic-load.exe   # Windows：不链接 -lswgreet
./dynamic-load.exe              # greet = Hello, Dynamic! / double(21) = 42
```

## DLL 链接调用（导入库 + 头文件）

`swc build`（kind = dll）自动产出 dll + 导入库（`swgreet.lib`）+ 头文件
（`swgreet.h`），可以像普通库一样 include + 链接：

```c
#include "swgreet.h"
sw_string name = { "Linked", 6 };
printf("%s\n", greet(&name)->data);       // Hello, Linked!
printf("%lld\n", twice(21));              // 42
```

```bash
clang ctest.c -I. -L. -lswgreet            # Windows 链接导入库
```

## 静态库配套头文件

`swc build`（kind = lib）会自动生成同名 C 头文件（`swmath.lib` → `swmath.h`）
并内附转发 stub，头文件按**用户函数名**直接声明，include + 链接即用：

```c
#include "swmath.h"
printf("%lld\n", add(5, 7));   // 12
```

> 头文件/导出集合以源码 `export function` 为准；导出函数名请避开 C 关键字
> （如 `double`/`int`），否则生成的 C 头文件无法编译。
