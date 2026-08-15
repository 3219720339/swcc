import { println, flush } from "std/io";

// 泛型 enum + 可选链探针：
//  1) 泛型函数返回泛型 enum 实例（Option<T> 返回值/签名，此前未专门验证）
//  2) 泛型函数参数为泛型 enum
//  3) 可选链 ?. 与 ?.[] 的语义
function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

enum Option<T> {
    Some(T),
    None,
}

// 泛型函数：返回 Option<T>（T 类型参数贯穿到返回值）
function maybe_int(value: int, flag: bool): Option<int> {
    if (flag) {
        return Option.Some(value);
    }
    return Option.None;
}

// 泛型函数：参数为 Option<T>，返回 T 的默认替代
function unwrap_or<T>(option: Option<T>, fallback: T): T {
    match (option) {
        Some(v) => {
            return v;
        }
        None => {
            return fallback;
        }
    }
}

// 泛型函数：接受 Option<string>
function describe(option: Option<string>): string {
    match (option) {
        Some(v) => {
            return "got:" + v;
        }
        None => {
            return "none";
        }
    }
}

function main(): int {
    let passed = 1;

    // ---------- 泛型 enum 返回值 ----------
    const s = maybe_int(42, true);
    const n = maybe_int(42, false);
    passed = passed & check(unwrap_or(s, 0) == 42, "generic fn returns Option.Some");
    passed = passed & check(unwrap_or(n, 7) == 7, "generic fn returns Option.None");

    // 泛型函数参数为泛型 enum（string 实例）
    passed = passed & check(describe(Option.Some("hi")) == "got:hi", "generic enum param Some");
    const none_str: Option<string> = Option.None;
    passed = passed & check(describe(none_str) == "none", "generic enum param None");

    // unwrap_or 类型参数推断：int / string（变量先类型化）
    passed = passed & check(unwrap_or(Option.Some("abc"), "def") == "abc", "unwrap_or string Some");
    const none_def: Option<string> = Option.None;
    passed = passed & check(unwrap_or(none_def, "def") == "def", "unwrap_or string None");

    // 嵌套泛型 enum（Option<Option<int>>）
    const outer: Option<Option<int>> = Option.Some(Option.Some(9));
    match (outer) {
        Some(inner) => {
            match (inner) {
                Some(v) => {
                    passed = passed & check(v == 9, "nested generic enum value");
                }
                None => {
                    passed = passed & check(false, "nested inner should be Some");
                }
            }
        }
        None => {
            passed = passed & check(false, "nested outer should be Some");
        }
    }

    // 泛型 enum 作为返回值 + match 表达式
    const doubled = match (maybe_int(5, true)) {
        Some(v) => v * 2,
        None => 0,
    };
    passed = passed & check(doubled == 10, "generic enum in match expression");

    // ---------- 可选链 ----------
    // class + nullable：?.value 在 null 上返回 null，?? 兜底
    const node: Node? = new Node(7);
    const a = node?.value ?? 0;
    passed = passed & check(a == 7, "optional chain on non-null reads field");

    const missing: Node? = null;
    const b = missing?.value ?? 0;
    passed = passed & check(b == 0, "optional chain on null falls back");

    // 可选链 + 方法调用
    const c = node?.get_doubled() ?? -1;
    passed = passed & check(c == 14, "optional chain method call");

    const d = missing?.get_doubled() ?? -1;
    passed = passed & check(d == -1, "optional chain method on null falls back");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    flush();
    return passed == 1 ? 0 : 1;
}

class Node {
    value: int;
    constructor(v: int) {
        this.value = v;
    }
    get_doubled(): int {
        return this.value * 2;
    }
}
