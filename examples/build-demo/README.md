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

> 说明：`swcc.toml` 会在入口文件所在目录向上查找；dll 导出的符号目前是
> Sw 内部 stable 名（`sw_fn_<模块>_<名>_<位置>`），用户友好名映射待后续版本。

## DLL 动态加载（不需要 .lib）

dll 本身支持运行时动态加载（LoadLibrary/dlopen + GetProcAddress/dlsym），
不需要链接期导入库。见 `greet-dll/dynamic-load.c`：

```bash
cd examples/build-demo/greet-dll
swc build greeter.sw            # 生成 swgreet.dll
clang dynamic-load.c -o dynamic-load.exe   # Windows：不链接 -lswgreet
./dynamic-load.exe              # greet = Hello, Dynamic! / double(21) = 42
```

## 静态库配套头文件

`swc build`（kind = lib）会自动生成同名 C 头文件（`swmath.lib` → `swmath.h`），
声明全部导出函数的 C 签名（含 sw_string/sw_array 布局），可直接 include：

```c
#include "swmath.h"
printf("%lld\n", sw_fn_0_add_55(5, 7));   // 12
```

> 头文件用 Sw stable 符号名；源码函数名在声明行尾注释标注。
