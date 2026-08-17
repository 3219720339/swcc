import { println } from "std/io";

class Boom { v: int; constructor(x: int) { this.v = x; } }

let g_log = "";

function f1(): int {
    try {
        return 42;
    } finally {
        g_log = g_log + "f1;";
    }
}

function f4(): int {
    try {
        throw new Boom(1);
    } catch (e) {
        return 7;
    } finally {
        g_log = g_log + "f4-fin;";
    }
}

function f5(): int {
    try {
        try {
            return 1;
        } finally {
            g_log = g_log + "inner;";
        }
    } finally {
        g_log = g_log + "outer;";
    }
}

function check(c: bool, label: string): int {
    if (c) { println(`[ok] ${label}`); return 1; }
    println(`[FAIL] ${label}`);
    return 0;
}

function main(): int {
    let passed = 1;

    // 1) return 保持值 + finally 执行
    g_log = "";
    const r1 = f1();
    passed = passed & check(r1 == 42 && g_log == "f1;", "return value kept + finally");

    // 2) break 在 try 里（循环内）
    g_log = "";
    for (let i = 0; i < 5; i++) {
        try {
            if (i == 2) { break; }
        } finally {
            g_log = g_log + `b${i};`;
        }
    }
    passed = passed & check(g_log == "b0;b1;b2;", "break runs finally");

    // 3) throw 不匹配 → finally → 外部 catch
    g_log = "";
    try {
        try {
            throw new Boom(1);
        } finally {
            g_log = g_log + "inner-fin;";
        }
    } catch (e) {
        g_log = g_log + "caught;";
    }
    passed = passed & check(g_log == "inner-fin;caught;", "throw finally then catch");

    // 4) catch 内 return + finally
    g_log = "";
    const r4 = f4();
    passed = passed & check(r4 == 7 && g_log == "f4-fin;", "catch return + finally");

    // 5) 嵌套 try/finally（内层 return 触发两层 finally）
    g_log = "";
    const r5 = f5();
    passed = passed & check(r5 == 1 && g_log == "inner;outer;", "nested finally both run");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
