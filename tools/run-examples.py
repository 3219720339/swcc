#!/usr/bin/env python3
"""跨平台 examples 回归：逐个 swc run，断言退出码等于探针设计值。

用法: python tools/run-examples.py [swc 可执行文件路径]
默认路径: target/release/swc（Windows 上自动补 .exe）
"""

import os
import subprocess
import sys

EXPECTED = {
    "fib.sw": 0,
    "hello.sw": 0,
    "lambda-probe.sw": 42,
    "probe-argv.sw": 0,
    "probe-bool.sw": 0,
    "probe-array.sw": 10,
    "probe-array-desc.sw": 0,
    "probe-adt.sw": 0,
    "probe-chmod.sw": 0,
    "probe-circular.sw": 0,
    "probe-closure.sw": 42,
    "probe-closure2.sw": 77,
    "probe-cn.sw": 0,
    "probe-dir.sw": 0,
    "probe-destructure.sw": 0,
    "probe-exc-2.sw": 1,
    "probe-exc-3.sw": 1,
    "probe-exc-debug.sw": 1,
    "probe-exc-nested.sw": 1,
    "probe-exc-nothrow.sw": 42,
    "probe-exc-setup.sw": 0,
    "probe-enum-c.sw": 0,
    "probe-exceptions.sw": 42,
    "probe-format.sw": 0,
    "probe-gc.sw": 0,
    "probe-hash.sw": 0,
    "probe-bound-inherit.sw": 0,
    "probe-bound-reverse.sw": 0,
    "probe-generic.sw": 0,
    "probe-generic-iface.sw": 0,
    "probe-if.sw": 42,
    "probe-inherit.sw": 0,
    "probe-interface.sw": 0,
    "probe-io.sw": 0,
    "probe-json.sw": 0,
    "probe-multifile.sw": 0,
    "probe-match-expr.sw": 0,
    "probe-map-clear.sw": 0,
    "probe-map-values.sw": 0,
    "probe-net.sw": 0,
    "probe-overload.sw": 0,
    "probe-param-if.sw": 3,
    "probe-param.sw": 42,
    "probe-process.sw": 0,
    "probe-println.sw": 0,
    "probe-return.sw": 42,
    "probe-slice.sw": 0,
    "probe-slice-u8.sw": 0,
    "probe-result.sw": 0,
    "probe-string.sw": 0,
    "probe-stdlib.sw": 0,
    "probe-stdlib2.sw": 0,
    "probe-stdlib3.sw": 0,
    "probe-stdlib4.sw": 0,
    "probe-spread.sw": 0,
    "probe-array-methods.sw": 0,
    "probe-struct.sw": 0,
    "probe-struct2.sw": 0,
    "probe-struct3.sw": 0,
    "probe-static.sw": 0,
    "probe-template.sw": 0,
    "probe-template-escape.sw": 0,
    "probe-ternary.sw": 43,
    "probe-test.sw": 0,
    "probe-text.sw": 0,
    "probe-time.sw": 0,
    "probe-regex.sw": 0,
    "probe-map-typed.sw": 0,
    "probe-flags.sw": 0,
    "probe-url.sw": 0,
    "probe-hash2.sw": 0,
    "probe-http.sw": 0,
    "probe-util2.sw": 0,
    "probe-cross-stdlib.sw": 0,
    "probe-batch3.sw": 0,
    "probe-batch4.sw": 0,
    "probe-trait.sw": 0,
    "probe-unicode.sw": 0,
    "probe-vars.sw": 0,
    "showcase.sw": 0,
}

# 需要额外命令行参数的探针：name -> [参数...]
ARGS = {
    "probe-flags.sw": ["--verbose", "--port=8080", "--host", "127.0.0.1", "--mode", "fast", "-v"],
}


def main() -> int:
    swc = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        "target", "release", "swc.exe" if os.name == "nt" else "swc"
    )
    failed = []
    for name, want in EXPECTED.items():
        path = os.path.join("examples", name)
        cmd = [swc, "run", path] + ARGS.get(name, [])
        try:
            proc = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                timeout=120,
            )
        except subprocess.TimeoutExpired:
            failed.append(f"{name}: 超时（120s）")
            if proc.stdout:
                print((proc.stdout or "")[-3000:])
            continue
        if proc.returncode != want:
            failed.append(f"{name}: 期望 {want}，实际 {proc.returncode}")
            tail = (proc.stdout or "")[-1500:] + (proc.stderr or "")[-1500:]
            print(tail)
    if failed:
        print("examples 回归失败：")
        for line in failed:
            print("  " + line)
        return 1
    print(f"examples 回归通过：{len(EXPECTED)} 个文件退出码全部符合设计")
    return 0


if __name__ == "__main__":
    sys.exit(main())
