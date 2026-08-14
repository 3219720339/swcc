import { println } from "std/io";

// 纯 C 风格枚举（无字段变体）——成员访问此前损坏。
enum Color {
    Red,
    Green,
    Blue,
}

// ADT 枚举（混合空/带字段变体）回归。
enum Option<T> {
    Some(T),
    None,
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
    const c: Color = Color.Red;
    // C 风格枚举可 match（全空变体分支）。
    let red_is_first = 0;
    match (c) {
        Red => { red_is_first = 1; }
        Green => { red_is_first = 0; }
        Blue => { red_is_first = 0; }
    }
    passed = passed & check(red_is_first == 1, "C-style enum member construct + match");

    // ADT 枚举仍正常。
    const some: Option<int> = Option.Some(5);
    const none: Option<int> = Option.None;
    let matched = 0;
    match (some) {
        Some(v) => { matched = v; }
        None => { matched = -1; }
    }
    let none_ok = 0;
    match (none) {
        Some(_) => { none_ok = 0; }
        None => { none_ok = 1; }
    }
    passed = passed & check(matched == 5 && none_ok == 1, "ADT enum still works");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
