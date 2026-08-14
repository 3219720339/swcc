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
    "probe-array.sw": 10,
    "probe-chmod.sw": 0,
    "probe-closure.sw": 42,
    "probe-closure2.sw": 77,
    "probe-dir.sw": 0,
    "probe-exc-2.sw": 1,
    "probe-exc-3.sw": 1,
    "probe-exc-debug.sw": 1,
    "probe-exc-nested.sw": 1,
    "probe-exc-nothrow.sw": 42,
    "probe-exc-setup.sw": 0,
    "probe-exceptions.sw": 42,
    "probe-format.sw": 0,
    "probe-generic.sw": 0,
    "probe-if.sw": 42,
    "probe-interface.sw": 0,
    "probe-io.sw": 0,
    "probe-json.sw": 0,
    "probe-net.sw": 0,
    "probe-param-if.sw": 3,
    "probe-param.sw": 42,
    "probe-process.sw": 0,
    "probe-println.sw": 0,
    "probe-return.sw": 42,
    "probe-string.sw": 0,
    "probe-stdlib.sw": 0,
    "probe-stdlib2.sw": 0,
    "probe-stdlib3.sw": 0,
    "probe-struct.sw": 0,
    "probe-template.sw": 0,
    "probe-template-escape.sw": 0,
    "probe-ternary.sw": 43,
}


def main() -> int:
    swc = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        "target", "release", "swc.exe" if os.name == "nt" else "swc"
    )
    failed = []
    for name, want in EXPECTED.items():
        path = os.path.join("examples", name)
        try:
            proc = subprocess.run(
                [swc, "run", path],
                capture_output=True,
                text=True,
                timeout=120,
            )
        except subprocess.TimeoutExpired:
            failed.append(f"{name}: 超时（120s）")
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
