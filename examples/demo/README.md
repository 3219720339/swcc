# Sw 语言演示（examples/demo）—— 真实输出版

本目录演示 **每个函数跑出来的真实结果**（不是 `[ok]` 断言）。
`demo-*.sw` 直接调用标准库并打印实际值，覆盖全部基础库模块；
完整运行输出保存在 `outputs/demo-*.txt`（含编译耗时）。

## 运行方式

```bash
swc run examples/demo/demo-time.sw     # 时间库
swc run examples/demo/demo-string.sw   # 字符串库
swc run examples/demo/demo-os.sw       # 系统/进程
```

## 演示清单（demo-*.sw，均为真实输出）

| 模块 | 演示内容 | 输出示例（节选） |
| --- | --- | --- |
| demo-print | **println 直接输出任意类型**（int/float/bool/char/string） | `42` `3.14` `true` `2026-08-15` |
| demo-time | 时间戳/字段/中文星期/时长/增减/间隔（直接输出） | `2026-08-15` `6 六` `00:01:30` |
| demo-string | 链式方法/格式化/解析/中文字符/中文函数名 | `reverse=wS好你` `format=score: 42 (3.14)` `format_float=3.14` |
| demo-math | 取整/三角/对数/随机/常量 | `sqrt(16)=4.0000` `hypot(3,4)=5.0000` `pi=3.1416` |
| demo-array | 排序/反转/极值/求和/去重/查找 | `sort_int=[1,2,3,4,5]` `sum_int=15` `unique=[a,b,c]` |
| demo-map | 键值增删查/长度/键值列表 | `keys=name,year,lang` `get_missing=(null)` |
| demo-json | 解析/取字段/数组/序列化 | `lang=sw` `year=2026` `stringify={...}` |
| demo-encoding | base64/hex/url/html 转义 | `base64=aGVsbG8g5L2g5aW9`（解码还原） |
| demo-unicode | 字符/字节转换、可打印判断 | `len=4 byte_len=8` `char_at(0)=20320` |
| demo-hash | FNV-1a / DJB2 哈希值 | `djb2("hello")=210714636441` |
| demo-fs | 文件读写/目录/路径/glob/walk | `lines=3` `file_size=29` `walk_files_count=2` |
| demo-os | 平台/目录/环境变量/子进程 | `platform=windows` `run=[hello-from-subprocess]` |
| demo-net | 本机 TCP 回环收发 | `listen_port=52843` `recv=hello-from-sw` |

> demo-time / demo-print 展示新能力：`println(任意类型)` 直接输出结果，
> 多参数之间用空格分隔，无需标签、拼接或模板字符串。

## 完整输出

`outputs/demo-*.txt` 保存每个演示的完整运行结果（含编译耗时）。

## 其余文件

目录里还复制了 `examples/` 的全部正式示例（probe-*.sw、hello.sw、
fib.sw、showcase.sw 等），它们是功能探针（断言式），供回归使用；
想看"真实数值输出"以 `demo-*.sw` 为准。
