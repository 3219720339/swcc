import { println } from "std/io";
import {
    regex_match,
    regex_find,
    regex_find_all,
    regex_replace,
    正则匹配,
    正则查找,
    正则查找全部,
    正则替换,
} from "std/regex";
import { join } from "std/string";

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

    // 字面量/点/量词
    passed = passed & check(regex_match("abc", "abc"), "literal");
    passed = passed & check(regex_match("abc", "a.c"), "dot");
    passed = passed & check(regex_match("ac", "ab*c"), "star zero");
    passed = passed & check(regex_match("abbc", "ab*c"), "star many");
    passed = passed & check(regex_match("abc", "ab+c"), "plus");
    passed = passed & check(regex_match("ac", "ab?c"), "quest zero");
    passed = passed & check(regex_match("abc", "ab?c"), "quest one");
    passed = passed & check(!regex_match("ac", "ab+c"), "plus reject");

    // 锚点
    passed = passed & check(regex_match("abc", "^abc$"), "anchor both");
    passed = passed & check(!regex_match("xabc", "^abc$"), "anchor reject");
    passed = passed & check(regex_match("abc", "^abc"), "anchor begin");
    passed = passed & check(regex_match("abc", "abc$"), "anchor end");

    // 字符类与转义
    passed = passed & check(regex_match("a5", "a[0-9]"), "class range");
    passed = passed & check(regex_match("ax", "a[^0-9]"), "class negate");
    passed = passed & check(regex_match("a5", "a\\d"), "digit");
    passed = passed & check(regex_match("a_", "a\\w"), "word");
    passed = passed & check(regex_match("a ", "a\\s"), "space");
    passed = passed & check(regex_match("a.b", "a\\.b"), "escaped dot");

    // 分组与交替
    passed = passed & check(regex_match("ab", "(ab)+"), "group plus");
    passed = passed & check(regex_match("cat", "cat|dog"), "alt first");
    passed = passed & check(regex_match("dog", "cat|dog"), "alt second");
    passed = passed & check(regex_match("abab", "(ab)+"), "group repeat");

    // find / find_all
    passed = passed & check(regex_find("订单 #1024 已发货", "\\d+") == "1024", "find digit");
    passed = passed & check(regex_find("no number", "\\d+") == "", "find none");
    const all = regex_find_all("a1 b22 c333", "\\d+");
    passed = passed & check(all.length == 3 && all[0] == "1" && all[1] == "22" && all[2] == "333", "find_all");
    const cns = 正则查找全部("你好123世界456", "\\d+");
    passed = passed & check(cns.length == 2 && cns[0] == "123" && cns[1] == "456", "cn find_all chinese");

    // replace
    passed = passed & check(regex_replace("2026-08-15", "-", "/") == "2026/08/15", "replace");
    passed = passed & check(regex_replace("a1b2", "\\d", "#") == "a#b#", "replace digits");
    passed = passed & check(regex_replace("订单#1024", "\\d+", "[$0]") == "订单#[1024]", "replace keep $0");
    passed = passed & check(正则替换("a b c", " ", "_") == "a_b_c", "cn replace");

    // 中文安全
    passed = passed & check(regex_match("你好，世界", "你好.*世界"), "chinese literal");
    passed = passed & check(正则匹配("hello world", "hello.*world"), "cn match");
    passed = passed & check(正则查找("abc123def", "\\d+") == "123", "cn find");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
