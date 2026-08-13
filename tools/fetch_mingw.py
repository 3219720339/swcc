#!/usr/bin/env python3
"""从 MSYS2 镜像下载 MinGW-w64 CRT 静态库，打包进 tools/mingw。

用法：python tools/fetch_mingw.py
产物：tools/mingw/lib/*.a、crt2.o（供 swc 用 lld 链接，用户无需安装任何工具链）
"""

import io
import os
import re
import sys
import tarfile
import urllib.request

import zstandard

BASE = "https://mirrors.huaweicloud.com/msys2/mingw/mingw64/"
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "mingw", "lib")
PKGS = ["crt", "gcc-runtime", "winpthreads"]


def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": "swcc-tools"})
    with urllib.request.urlopen(request, timeout=120) as response:
        return response.read()


def latest_package(kind):
    html = fetch(BASE).decode("utf-8", "replace")
    pattern = re.compile(
        r'href="(mingw-w64-x86_64-'
        + re.escape(kind)
        + r'-[^"]*\.pkg\.tar\.zst)"'
    )
    names = [
        match.group(1)
        for match in pattern.finditer(html)
        if "-git-" not in match.group(1) and "-stub-" not in match.group(1)
    ]
    return names[-1] if names else None


def extract_libs(package):
    print(f"下载 {package}")
    data = fetch(BASE + package)
    decompressor = zstandard.ZstdDecompressor()
    stream = decompressor.stream_reader(io.BytesIO(data))
    with tarfile.open(fileobj=stream, mode="r|") as archive:
        for member in archive:
            name = member.name.replace("\\", "/")
            if not member.isfile():
                continue
            if name.endswith(".a") or name.endswith(".o"):
                basename = os.path.basename(name)
                target = os.path.join(OUT, basename)
                source = archive.extractfile(member)
                if source is None:
                    continue
                with open(target, "wb") as output:
                    output.write(source.read())
                print(f"  -> {basename}")


def main():
    os.makedirs(OUT, exist_ok=True)
    for kind in PKGS:
        package = latest_package(kind)
        if package is None:
            print(f"未找到 {kind} 包", file=sys.stderr)
            sys.exit(1)
        extract_libs(package)
    print("完成：", OUT)


if __name__ == "__main__":
    main()
