// 整数宽度显式转换（as）按位截断/扩展语义（bug #1 修复）。
// 文档承诺"显式截断/扩展"、"按位重新解释宽度转换"：
//   300 as u8 → 44（取低 8 位无符号）
//   200 as i8 → -56（取低 8 位有符号）
//   65536 as u16 → 0、70000 as i16 → 4464
//   (-1) as u8 → 255、-255 as i8 → 1
import { println } from "std/io";

function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

function main(): int {
    let passed = 1;
    passed = passed & check((300 as u8) == 44, "300 as u8 -> 44");
    passed = passed & check(((200 as i8) as int) == -56, "200 as i8 -> -56");
    passed = passed & check((65536 as u16) == 0, "65536 as u16 -> 0");
    passed = passed & check(((70000 as i16) as int) == 4464, "70000 as i16 -> 4464");
    passed = passed & check((5 as u8) == 5, "5 as u8 stays 5");
    passed = passed & check(((-1) as u8) == 255, "-1 as u8 -> 255");
    passed = passed & check(((-255) as i8) as int == 1, "-255 as i8 -> 1");
    passed = passed & check((42 as u64) == 42, "64-bit cast identity");
    passed = passed & check((3.9 as int) == 3, "float to int truncates");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
