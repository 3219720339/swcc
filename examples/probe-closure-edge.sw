// 闭包边界回归：嵌套闭包跨层捕获、字符串 for-of（UTF-8 字符数）、
// lambda 返回函数类型、函数类型注解歧义。
import { println } from "std/io";

function make_adder<T>(base: T): (T) => T {
    return (x: T): T => x;
}

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

    // 嵌套闭包：内层捕获外层 lambda 体局部 + 父函数局部
    const a = 10;
    const outer = (): (() => int) => {
        const b = 20;
        return (): int => a + b;
    };
    passed = passed & check(outer()() == 30, "nested closure two-level capture");

    // 字符串 for-of 按字符迭代（UTF-8 多字节）
    const cn = "你好世界";
    let count = 0;
    let chars = "";
    for (const ch of cn) {
        count = count + 1;
        chars = chars + ch;
    }
    passed = passed & check(count == 4 && chars == "你好世界", "string for-of utf8 chars");

    // lambda 返回函数类型注解（括号分组歧义修复）
    const make = (): (() => int) => () => 42;
    passed = passed & check(make()() == 42, "lambda ret function type");

    // 泛型函数返回函数类型
    passed = passed & check(make_adder(5)(7) == 7, "generic fn ret function type");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
