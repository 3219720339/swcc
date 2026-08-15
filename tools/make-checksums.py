#!/usr/bin/env python3
"""生成/校验 SDK 产物的 SHA-256 校验和清单。

用法：
  python3 tools/make-checksums.py <目录>            # 生成 <目录>/SHA256SUMS
  python3 tools/make-checksums.py <目录> --check    # 校验现有 SHA256SUMS
  python3 tools/make-checksums.py --version <目录>  # 只列出 <目录> 下含版本号的产物

约定：
  - 扫描目录下所有文件（含子目录），对每个文件计算 SHA-256；
  - 写入 <目录>/SHA256SUMS，每行 `哈希  <相对路径>`（两空格，GNU 风格）；
  - --check 时按 SHA256SUMS 内容逐条校验，输出不一致的文件并以非零退出。
"""

import argparse
import hashlib
import os
import sys

BLOCK = 1 << 20


def sha256_of(path: str) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        while True:
            chunk = f.read(BLOCK)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def list_files(root: str):
    for folder, _, files in os.walk(root):
        for name in files:
            full = os.path.join(folder, name)
            rel = os.path.relpath(full, root)
            yield full, rel


def generate(root: str) -> int:
    sums_path = os.path.join(root, "SHA256SUMS")
    lines = []
    for full, rel in sorted(list_files(root)):
        if rel == "SHA256SUMS":
            continue
        lines.append(f"{sha256_of(full)}  {rel}")
    with open(sums_path, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print(f"已生成 {sums_path}（{len(lines)} 个文件）")
    return 0


def verify(root: str) -> int:
    sums_path = os.path.join(root, "SHA256SUMS")
    if not os.path.exists(sums_path):
        sys.exit(f"缺少校验和文件：{sums_path}")
    failed = 0
    checked = 0
    with open(sums_path, encoding="utf-8") as f:
        for lineno, line in enumerate(f, 1):
            line = line.rstrip("\n")
            if not line.strip():
                continue
            try:
                digest, rel = line.split("  ", 1)
            except ValueError:
                print(f"[行 {lineno}] 格式错误：{line!r}")
                failed += 1
                continue
            full = os.path.join(root, rel)
            if not os.path.exists(full):
                print(f"缺失文件：{rel}")
                failed += 1
                continue
            actual = sha256_of(full)
            checked += 1
            if actual != digest:
                print(f"校验失败：{rel}")
                print(f"  期望 {digest}")
                print(f"  实际 {actual}")
                failed += 1
    print(f"校验完成：{checked} 个文件，{failed} 个不一致")
    return 1 if failed else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", nargs="?", default="dist",
                        help="产物目录（默认 dist）")
    parser.add_argument("--check", action="store_true",
                        help="校验既有 SHA256SUMS 而非生成")
    parser.add_argument("--version", action="store_true",
                        help="仅列出含版本号的产物文件")
    args = parser.parse_args()

    if not os.path.isdir(args.directory):
        sys.exit(f"目录不存在：{args.directory}")

    if args.version:
        for full, rel in sorted(list_files(args.directory)):
            base = os.path.basename(rel)
            if any(ch.isdigit() for ch in base) and ("swc-" in base or base.endswith(".zip") or base.endswith(".tar.gz")):
                print(full)
        return 0

    if args.check:
        return verify(args.directory)
    return generate(args.directory)


if __name__ == "__main__":
    sys.exit(main())
