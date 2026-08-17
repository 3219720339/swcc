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

    // ① string switch 内容比较（此前静默走 default）
    let r = 0;
    const s = "b";
    switch (s) {
        case "a": r = 1; break;
        case "b": r = 2; break;
        case "c": r = 3; break;
        default: r = 9;
    }
    passed = passed & check(r == 2, "string switch match");
    const s2 = "zzz";
    switch (s2) {
        case "a": r = 1; break;
        case "b": r = 2; break;
        default: r = 9;
    }
    passed = passed & check(r == 9, "string switch default");

    // ② ?? 类型收窄 + ??= 可用
    let a: int? = null;
    const v = a ?? 5;
    let w: int = v;
    passed = passed & check(w == 5, "?? unwrap int");
    a ??= 9;
    passed = passed & check((a ?? 0) == 9, "??= int");
    let n: int? = 7;
    n ??= 3;
    passed = passed & check((n ?? 0) == 7, "??= non-null keeps");
    let st: string? = null;
    st ??= "fallback";
    passed = passed & check((st ?? "") == "fallback", "??= string");

    // ③ do-while：基本 + break + continue
    let i = 0;
    let sum = 0;
    do {
        i = i + 1;
        sum = sum + i;
    } while (i < 3);
    passed = passed & check(sum == 6, "do-while basic");
    let j = 0;
    do {
        j = j + 1;
        if (j == 2) { break; }
    } while (true);
    passed = passed & check(j == 2, "do-while break");
    let k = 0;
    let cnt = 0;
    do {
        k = k + 1;
        if (k % 2 == 0) { continue; }
        cnt = cnt + 1;
    } while (k < 5);
    passed = passed & check(cnt == 3, "do-while continue");
    let zero = 0;
    do {
        zero = 1;
    } while (zero < 0);
    passed = passed & check(zero == 1, "do-while runs body first");

    // ④ 字符串方法 JS 别名
    const t = "Hello, World";
    passed = passed & check(t.toLowerCase() == "hello, world", "toLowerCase");
    passed = passed & check(t.toUpperCase() == "HELLO, WORLD", "toUpperCase");
    passed = passed & check(t.startsWith("Hello"), "startsWith");
    passed = passed & check(t.endsWith("World"), "endsWith");
    passed = passed & check(t.includes("lo, Wo"), "includes");
    passed = passed & check(t.indexOf("l") == 2, "indexOf");
    passed = passed & check(t.slice(0, 5) == "Hello", "slice end-exclusive");
    passed = passed & check(t.slice(-5) == "World", "slice negative start");
    passed = passed & check("x".padStart(3, "0") == "00x", "padStart");
    passed = passed & check("x".padEnd(3, "!") == "x!!", "padEnd");
    passed = passed & check(t.charAt(1) == "e", "charAt");
    passed = passed & check(t.charCodeAt(0) == 72, "charCodeAt");

    // ⑤ 字符串大小比较（字典序）
    passed = passed & check("abc" < "abd", "string lt");
    passed = passed & check("abc" <= "abc", "string le");
    passed = passed & check("xyz" > "abc", "string gt");
    passed = passed & check("abc" >= "abd" == false, "string ge false");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
