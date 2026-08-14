import { println } from "std/io";
import { extract_between, char_code, replace_pairs } from "std/string";

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
    passed = passed & check("   ".is_blank(), "is_blank spaces");
    passed = passed & check("".is_blank(), "is_blank empty");
    passed = passed & check(" 你好 ".is_blank() == false, "is_blank false");
    passed = passed & check("a b\tc\n".strip_whitespace() == "abc", "strip_whitespace");
    passed = passed & check("你好，火山".substring_between("你", "火") == "好，", "between chinese");
    const bl = "a<b>c<d>".substring_between_last("<", ">");
    passed = passed & check(bl == "c", "between_last");
    passed = passed & check("no marker".substring_between("x", "y") == "", "between missing");

    const parts = extract_between("a[1]b[2]c", "[", "]");
    passed = passed & check(parts.length == 2 && parts[0] == "1" && parts[1] == "2", "extract_between");

    passed = passed & check("a-b-c".before("-") == "a", "before");
    passed = passed & check("a-b-c".after("-") == "b-c", "after");
    passed = passed & check("a-b-c".before_last("-") == "a-b", "before_last");
    passed = passed & check("a-b-c".after_last("-") == "c", "after_last");
    passed = passed & check("abc".before("x") == "", "before missing");

    passed = passed & check(char_code("A", 0) == 65, "char_code ascii");
    passed = passed & check(char_code("你", 0) == 20320, "char_code chinese");

    const replaced = replace_pairs("你好，火山", "你好", "Hello", "火山", "火山中文编程");
    passed = passed & check(replaced == "Hello，火山中文编程", "replace_pairs chinese");
    passed = passed & check(replace_pairs("a1a2", "a", "x", "1", "9") == "x9x2", "replace_pairs sequence");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
