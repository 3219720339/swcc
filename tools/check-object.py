#!/usr/bin/env python3
"""校验生成对象/可执行文件的格式与机器类型。
用法：python tools/check-object.py <文件> <期望>（期望：elf-aarch64 | macho）
"""

import struct
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.stderr.reconfigure(encoding="utf-8", errors="replace")


def main():
    path, expected = sys.argv[1], sys.argv[2]
    with open(path, "rb") as handle:
        data = handle.read()
    if expected == "elf-aarch64":
        assert data[:4] == b"\x7fELF", "应为 ELF"
        machine = struct.unpack("<H", data[18:20])[0]
        assert machine == 183, f"应为 AArch64 (183)，实际 {machine}"
    elif expected == "macho":
        assert data[:4] == b"\xcf\xfa\xed\xfe", "应为 Mach-O 64 位小端"
    else:
        sys.exit(f"未知期望格式：{expected}")
    print(f"OK: {path} ({expected})")


if __name__ == "__main__":
    main()
