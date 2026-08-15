import { println, flush } from "std/io";

// 空值窄化探针：if (x != null) / if (x == null) / 三元表达式的分支内
// 可空变量应窄化为非空类型，允许直接成员访问。
function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
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

function main(): int {
    let passed = 1;

    // ---------- if (x != null) then 窄化 ----------
    const node: Node? = new Node(7);
    let got = 0;
    if (node != null) {
        got = node.value;  // 窄化后应允许直接访问
    }
    passed = passed & check(got == 7, "if (x != null) narrows then branch");

    // if (x != null) 内方法调用
    let got_method = 0;
    if (node != null) {
        got_method = node.get_doubled();
    }
    passed = passed & check(got_method == 14, "if (x != null) method call");

    // ---------- if (x == null) else 窄化 ----------
    const missing: Node? = null;
    let got_else = -1;
    if (missing == null) {
        got_else = 0;
    } else {
        got_else = missing.value;  // else 分支：missing 非空，应允许
    }
    passed = passed & check(got_else == 0, "if (x == null) else narrows");

    // 反向：if (node == null) 的 else 分支
    let got_else2 = -1;
    if (node == null) {
        got_else2 = 0;
    } else {
        got_else2 = node.value;
    }
    passed = passed & check(got_else2 == 7, "if (x == null) else with non-null x");

    // ---------- 三元表达式窄化 ----------
    const via_ternary = node != null ? node.value : -1;
    passed = passed & check(via_ternary == 7, "ternary x != null narrows then");

    const via_ternary2 = missing != null ? missing.value : -1;
    passed = passed & check(via_ternary2 == -1, "ternary with null x falls back");

    // ---------- 参数窄化（可空参数） ----------
    const from_fn = use_node(node);
    passed = passed & check(from_fn == 7, "param narrowing in function");

    const from_fn2 = use_node(null);
    passed = passed & check(from_fn2 == -1, "param null passes through");

    // ---------- while 窄化 ----------
    let count = 0;
    let cursor: Node? = new Node(3);
    while (cursor != null) {
        count = count + cursor.value;  // 循环体内窄化
        cursor = null;
    }
    passed = passed & check(count == 3, "while (x != null) narrows body");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    flush();
    return passed == 1 ? 0 : 1;
}

function use_node(n: Node?): int {
    if (n != null) {
        return n.value;
    }
    return -1;
}
