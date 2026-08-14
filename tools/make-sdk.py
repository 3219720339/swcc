#!/usr/bin/env python3
"""构建 Windows x64 SDK：swc.exe + lld + MinGW(UCRT) 静态库 + 预编译运行时。

用法：python tools/make-sdk.py [llvm-mingw 目录] [输出目录]
产物：<输出>/swc-windows-x64.zip（解压即用，用户无需安装任何工具链）
"""

import os
import shutil
import subprocess
import sys
import zipfile

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.stderr.reconfigure(encoding="utf-8", errors="replace")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_TOOLCHAIN = r"D:\llvm-mingw-20260616-ucrt-x86_64"


def run(command, cwd=None):
    print("+", " ".join(str(part) for part in command))
    subprocess.run(command, cwd=cwd, check=True)


def main():
    toolchain = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_TOOLCHAIN
    out_root = sys.argv[2] if len(sys.argv) > 2 else os.path.join(ROOT, "dist")
    clang = os.path.join(toolchain, "bin", "clang.exe")
    lld = os.path.join(toolchain, "bin", "ld.lld.exe")
    mingw_lib = os.path.join(toolchain, "x86_64-w64-mingw32", "lib")
    builtins = os.path.join(
        toolchain, "lib", "clang", "22", "lib", "windows",
        "libclang_rt.builtins-x86_64.a",
    )
    for path in (clang, lld, mingw_lib, builtins):
        if not os.path.exists(path):
            sys.exit(f"工具链缺失：{path}")

    # 1) 发布版 swc
    run(["cargo", "build", "--release", "-p", "swc"], cwd=ROOT)
    swc = os.path.join(ROOT, "target", "release", "swc.exe")

    # 2) 组装 SDK 目录
    sdk = os.path.join(out_root, "swc-windows-x64")
    for sub in ("bin", "lib", "stdlib"):
        os.makedirs(os.path.join(sdk, sub), exist_ok=True)
    shutil.copy2(swc, os.path.join(sdk, "swc.exe"))
    shutil.copy2(lld, os.path.join(sdk, "bin", "ld.lld.exe"))
    for dll in ("libLLVM-22.dll", "libc++.dll", "libunwind.dll"):
        source = os.path.join(toolchain, "bin", dll)
        if os.path.exists(source):
            shutil.copy2(source, os.path.join(sdk, "bin", dll))
    for name in ("libucrt.a", "libucrtbase.a", "libkernel32.a"):
        shutil.copy2(os.path.join(mingw_lib, name), os.path.join(sdk, "lib", name))
    shutil.copy2(builtins, os.path.join(sdk, "lib", "libclang_rt.builtins-x86_64.a"))

    # 3) 预编译运行时（不再需要 clang）
    target = "x86_64-w64-windows-gnu"
    for source, output in (
        ("runtime.c", "runtime.obj"),
        ("runtime_x64.S", "runtime_asm.obj"),
        ("startup.s", "startup.obj"),
    ):
        src = os.path.join(ROOT, "runtime", source)
        dst = os.path.join(sdk, "lib", output)
        command = [clang, "-target", target, "-O2", "-c", src, "-o", dst]
        if source != "runtime.c":
            command = [clang, "-target", target, "-c", src, "-o", dst]
        run(command)

    # 4) 标准库
    stdlib_src = os.path.join(ROOT, "stdlib")
    for name in os.listdir(stdlib_src):
        shutil.copy2(os.path.join(stdlib_src, name), os.path.join(sdk, "stdlib", name))

    # 5) 打包 zip
    archive = os.path.join(out_root, "swc-windows-x64.zip")
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as zf:
        for folder, _, files in os.walk(sdk):
            for name in files:
                full = os.path.join(folder, name)
                relative = os.path.relpath(full, out_root)
                zf.write(full, relative)
    print(f"SDK 完成：{archive}")


if __name__ == "__main__":
    main()
