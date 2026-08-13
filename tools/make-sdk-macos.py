#!/usr/bin/env python3
"""构建 macOS 解压即用 SDK：swc + stdlib（原生链接用系统 cc，无需工具链）。
用法：python3 tools/make-sdk-macos.py <输出目录>
产物：<输出>/swc-macos.zip
"""

import os
import shutil
import subprocess
import sys
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def main():
    out_root = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "dist")
    subprocess.run(["cargo", "build", "--release", "-p", "swc"], cwd=ROOT, check=True)
    sdk = os.path.join(out_root, "swc-macos")
    os.makedirs(os.path.join(sdk, "stdlib"), exist_ok=True)
    shutil.copy2(os.path.join(ROOT, "target", "release", "swc"), os.path.join(sdk, "swc"))
    for name in os.listdir(os.path.join(ROOT, "stdlib")):
        shutil.copy2(os.path.join(ROOT, "stdlib", name), os.path.join(sdk, "stdlib", name))
    archive = os.path.join(out_root, "swc-macos.zip")
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as zf:
        for folder, _, files in os.walk(sdk):
            for name in files:
                full = os.path.join(folder, name)
                zf.write(full, os.path.relpath(full, out_root))
    print(f"SDK 完成：{archive}")


if __name__ == "__main__":
    main()
