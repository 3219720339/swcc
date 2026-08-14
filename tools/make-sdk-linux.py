#!/usr/bin/env python3
"""构建 Linux x64 解压即用 SDK：swc + lld + musl 静态库 + 预编译运行时。
用法：python3 tools/make-sdk-linux.py <llvm-mingw 目录> <输出目录>
产物：<输出>/swc-linux-x64-<版本号>.tar.gz（无需安装任何工具链即可静态链接 Linux 目标）
版本号取自仓库根 Cargo.toml 的 workspace.package.version。
"""

import os
import shutil
import subprocess
import sys
import tarfile
import re

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.stderr.reconfigure(encoding="utf-8", errors="replace")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TRIPLES = ["x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl"]


def read_version():
    with open(os.path.join(ROOT, "Cargo.toml"), encoding="utf-8") as f:
        for line in f:
            match = re.match(r'\s*version\s*=\s*"([^"]+)"', line)
            if match:
                return match.group(1)
    return "0.0.0"


def run(command, cwd=None):
    print("+", " ".join(str(part) for part in command))
    subprocess.run(command, cwd=cwd, check=True)


def rustup_rustlib(triple):
    home = os.environ.get("RUSTUP_HOME") or os.path.join(os.path.expanduser("~"), ".rustup")
    toolchains = os.path.join(home, "toolchains")
    for name in sorted(os.listdir(toolchains)):
        candidate = os.path.join(toolchains, name, "lib", "rustlib", triple, "lib")
        if os.path.isdir(candidate):
            return candidate
    sys.exit(f"缺少 rustup 目标库：{triple}（请先 rustup target add {triple}）")


def find_builtins(lib_dir):
    for name in os.listdir(lib_dir):
        if name.startswith("libcompiler_builtins-") and name.endswith(".rlib"):
            return os.path.join(lib_dir, name)
    return None


def main():
    toolchain = sys.argv[1]
    out_root = sys.argv[2] if len(sys.argv) > 2 else os.path.join(ROOT, "dist")
    version = read_version()
    clang = os.path.join(toolchain, "bin", "clang")
    lld = os.path.join(toolchain, "bin", "ld.lld")
    for path in (clang, lld):
        if not os.path.exists(path):
            sys.exit(f"工具链缺少：{path}")

    run(["cargo", "build", "--release", "-p", "swc"], cwd=ROOT)
    swc = os.path.join(ROOT, "target", "release", "swc")

    sdk = os.path.join(out_root, "swc-linux-x64")
    for sub in ("bin", "lib", "stdlib"):
        os.makedirs(os.path.join(sdk, sub), exist_ok=True)
    with open(os.path.join(sdk, "version.txt"), "w", encoding="utf-8") as f:
        f.write(f"swc {version}\n")
    shutil.copy2(swc, os.path.join(sdk, "swc"))
    shutil.copy2(lld, os.path.join(sdk, "bin", "ld.lld"))

    # 预编译运行时（x86_64 + aarch64）
    for arch, asm in (("x86_64", "runtime_x64.S"), ("aarch64", "runtime_aarch64.s")):
        target = f"{arch}-unknown-linux-musl"
        run([clang, "-target", target, "-O2", "-c", os.path.join(ROOT, "runtime", "runtime.c"),
             "-o", os.path.join(sdk, "lib", f"runtime_{arch}.o")])
        run([clang, "-target", target, "-c", os.path.join(ROOT, "runtime", asm),
             "-o", os.path.join(sdk, "lib", f"runtime_asm_{arch}.o")])

    # musl 静态库
    for triple in TRIPLES:
        lib_dir = rustup_rustlib(triple)
        self_contained = os.path.join(lib_dir, "self-contained")
        musl_dir = os.path.join(sdk, "musl", triple)
        os.makedirs(musl_dir, exist_ok=True)
        for name in ("crt1.o", "crti.o", "crtn.o", "libc.a"):
            shutil.copy2(os.path.join(self_contained, name), os.path.join(musl_dir, name))
        builtins = find_builtins(lib_dir)
        if builtins is not None:
            shutil.copy2(builtins, os.path.join(musl_dir, os.path.basename(builtins)))

    for name in os.listdir(os.path.join(ROOT, "stdlib")):
        shutil.copy2(os.path.join(ROOT, "stdlib", name), os.path.join(sdk, "stdlib", name))

    archive = os.path.join(out_root, f"swc-linux-x64-{version}.tar.gz")
    with tarfile.open(archive, "w:gz") as tf:
        tf.add(sdk, arcname="swc-linux-x64")
    print(f"SDK 完成：{archive}")


if __name__ == "__main__":
    main()
