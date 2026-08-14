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

function main(): int {
    // 基本匹配
    println(regex_match("abc", "a.c"));
    println(regex_match("2026-08-15", "\\d\\d\\d\\d-\\d\\d-\\d\\d"));
    println(regex_match("abbbbc", "ab*c"));
    println(regex_match("hello world", "^hello.*world$"));
    println(regex_match("cat", "cat|dog"));
    println(regex_match("dog", "cat|dog"));

    // 查找
    println(regex_find("订单 #1024 已发货", "\\d+"));
    println(regex_find("no number here", "\\d+"));
    println(正则查找("abc123def456", "\\d+"));

    // 查找全部
    const all = regex_find_all("a1 b22 c333", "\\d+");
    println(all.length);
    println(join(all, ","));
    const cns = 正则查找全部("价格 12.5 元，数量 3 件", "\\d+");
    println(join(cns, "|"));

    // 替换
    println(regex_replace("2026-08-15", "-", "/"));
    println(regex_replace("a1b2c3", "\\d", "#"));
    println(regex_replace("订单#1024 金额#88", "\\d+", "[$0]"));
    println(正则替换("Hello World", "[a-z]", "X"));
    return 0;
}
