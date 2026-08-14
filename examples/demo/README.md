# Sw 语言演示（examples/demo）

本目录是 `examples/` 的演示副本：挑了一批有**真实输出**的示例（而非纯
`[ok]` 断言），逐个编译运行，输出结果保存在 `outputs/<名称>.txt`。

## 运行方式

```bash
swc run examples/demo/hello.sw
swc run examples/demo/showcase.sw
swc run examples/demo/probe-argv.sw Sw    # 带命令行参数
```

## 演示清单与输出速览

| 示例 | 演示内容 | 输出示例 |
| --- | --- | --- |
| hello.sw | 类、数组、for-of、模板字符串 | `hello Sw` / `sum = 10` |
| fib.sw | 递归函数 | `fib(10) = 55` |
| showcase.sw | 综合：类型/类/接口/闭包/泛型/JSON/异常 | `showcase=PASS` |
| probe-format.sw | 字符串反转、补零、格式化、随机数、时间 | `rev=wS好你 p1=00042 dt=2026-01-02 00:00:00` |
| probe-io.sw | 文件读写、按行读取、字符串解析 | `lines=4 first=one last=four` |
| probe-json.sw | JSON 解析与取值、UTF-8 长度 | `lang=sw year=2026 first=gc chars=5` |
| probe-string.sw | 链式字符串方法、中文索引 | `cleaned=HELLO, SW first=你 len=4` |
| probe-template.sw | 模板字符串插值 | `fib(10) = 42` |
| probe-dir.sw | 目录/文件操作：列出、改名、复制、删除 | `entries=2 [a.txt b.txt ]` |
| probe-process.sw | 子进程：run/spawn/wait/kill/poll | `status=7 wait=5 kill=0 poll=0` |
| probe-argv.sw | main(args) 命令行参数、环境变量 | `hello Sw` / `home=C:\Users\Administrator` |
| probe-unicode.sw | Unicode 字符/字节转换 | `utf8_char_at 你` / `final=PASS` |

## 输出文件

每个示例的完整运行输出在 `outputs/` 下同名 `.txt`，含编译耗时与程序输出：

- outputs/hello.txt
- outputs/fib.txt
- outputs/showcase.txt
- outputs/probe-format.txt
- outputs/probe-io.txt
- outputs/probe-json.txt
- outputs/probe-string.txt
- outputs/probe-template.txt
- outputs/probe-dir.txt
- outputs/probe-process.txt
- outputs/probe-argv.txt
- outputs/probe-unicode.txt
