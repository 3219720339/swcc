import { println } from "std/io";
import { utf8_len, utf8_char_at, utf8_byte_len, utf8_index_to_byte, utf8_byte_to_index, utf8_is_printable } from "std/unicode";

function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

// "你好A" = 你(3字节) 好(3字节) A(1字节) = 7 字节, 3 字符
function main(): int {
    let passed = 1;
    const s = "你好A";
    passed = passed & check(utf8_len(s) == 3, "utf8_len chars");
    passed = passed & check(utf8_byte_len(s) == 7, "utf8_byte_len bytes");
    passed = passed & check(utf8_char_at(s, 0) == 20320, "utf8_char_at 你");
    // 字节偏移：你=0, 好=3, A=6
    passed = passed & check(utf8_index_to_byte(s, 0) == 0, "index_to_byte 0 -> 0");
    passed = passed & check(utf8_index_to_byte(s, 1) == 3, "index_to_byte 1 -> 3");
    passed = passed & check(utf8_index_to_byte(s, 2) == 6, "index_to_byte 2 -> 6");
    passed = passed & check(utf8_index_to_byte(s, 3) == 7, "index_to_byte end -> len");
    passed = passed & check(utf8_index_to_byte(s, 4) == -1, "index_to_byte out of range");
    passed = passed & check(utf8_byte_to_index(s, 3) == 1, "byte_to_index 3 -> 1");
    passed = passed & check(utf8_byte_to_index(s, 6) == 2, "byte_to_index 6 -> 2");
    passed = passed & check(utf8_byte_to_index(s, 1) == -1, "byte_to_index mid-char -> -1");
    passed = passed & check(utf8_is_printable("hello 你好!") == true, "is_printable printable");
    passed = passed & check(utf8_is_printable("a\u{7}b") == false, "is_printable control char");
    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
