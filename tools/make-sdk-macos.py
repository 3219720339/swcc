#!/usr/bin/env python3
"""构建 macOS 解压即用 SDK：swc + stdlib（原生链接用系统 cc，无需工具链）。
用法：python3 tools/make-sdk-macos.py <输出目录>
产物：<输出>/swc-macos-<版本号>.zip
版本号取自仓库根 Cargo.toml 的 workspace.package.version。
"""

import os
import shutil
import subprocess
import sys
import zipfile
import re

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.stderr.reconfigure(encoding="utf-8", errors="replace")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def read_version():
    with open(os.path.join(ROOT, "Cargo.toml"), encoding="utf-8") as f:
        for line in f:
            match = re.match(r'\s*version\s*=\s*"([^"]+)"', line)
            if match:
                return match.group(1)
    return "0.0.0"


def main():
    out_root = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "dist")
    version = read_version()
    subprocess.run(["cargo", "build", "--release", "-p", "swc"], cwd=ROOT, check=True)
    sdk = os.path.join(out_root, "swc-macos")
    os.makedirs(os.path.join(sdk, "stdlib"), exist_ok=True)
    with open(os.path.join(sdk, "version.txt"), "w", encoding="utf-8") as f:
        f.write(f"swc {version}\n")
    shutil.copy2(os.path.join(ROOT, "target", "release", "swc"), os.path.join(sdk, "swc"))
    for name in os.listdir(os.path.join(ROOT, "stdlib")):
        shutil.copy2(os.path.join(ROOT, "stdlib", name), os.path.join(sdk, "stdlib", name))
    archive = os.path.join(out_root, f"swc-macos-{version}.zip")
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as zf:
        for folder, _, files in os.walk(sdk):
            for name in files:
                full = os.path.join(folder, name)
                zf.write(full, os.path.relpath(full, out_root))
    print(f"SDK 完成：{archive}")


if __name__ == "__main__":
    main()
