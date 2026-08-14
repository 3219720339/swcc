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
