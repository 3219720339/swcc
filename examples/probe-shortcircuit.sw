import { println, flush } from "std/io";

// 短路语义探针：验证 && / || 只按需求值右侧（副作用函数不应被调用），
// 以及结果值与 JS 语义一致。
function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

let side_effect_calls = 0;

function side_effect(): bool {
    side_effect_calls++;
    return true;
}

function fail(): bool {
    println("[FAIL] right side evaluated (should be short-circuited)");
    return true;
}

function main(): int {
    let passed = 1;

    // ---------- && 短路：false && x 不求值 x ----------
    side_effect_calls = 0;
    const a1 = false && side_effect();
    passed = passed & check(a1 == false, "false && x == false");
    passed = passed & check(side_effect_calls == 0, "&& short-circuits on false");

    // ---------- || 短路：true || x 不求值 x ----------
    side_effect_calls = 0;
    const a2 = true || side_effect();
    passed = passed & check(a2 == true, "true || x == true");
    passed = passed & check(side_effect_calls == 0, "|| short-circuits on true");

    // ---------- 正常路径仍求值右侧 ----------
    side_effect_calls = 0;
    const a3 = true && side_effect();
    passed = passed & check(a3 == true, "true && x evaluates right");
    passed = passed & check(side_effect_calls == 1, "&& evaluates right when left true");

    side_effect_calls = 0;
    const a4 = false || side_effect();
    passed = passed & check(a4 == true, "false || x evaluates right");
    passed = passed & check(side_effect_calls == 1, "|| evaluates right when left false");

    // ---------- 结果值语义（布尔） ----------
    passed = passed & check((true && true) == true, "true && true");
    passed = passed & check((true && false) == false, "true && false");
    passed = passed & check((false && true) == false, "false && true");
    passed = passed & check((false && false) == false, "false && false");
    passed = passed & check((true || true) == true, "true || true");
    passed = passed & check((true || false) == true, "true || false");
    passed = passed & check((false || true) == true, "false || true");
    passed = passed & check((false || false) == false, "false || false");

    // ---------- 嵌套短路 ----------
    side_effect_calls = 0;
    const n1 = (false && side_effect()) || side_effect();
    passed = passed & check(n1 == true, "nested short-circuit result");
    passed = passed & check(side_effect_calls == 1, "nested: only rightmost || evaluated");

    side_effect_calls = 0;
    const n2 = (true || side_effect()) && (false && side_effect());
    passed = passed & check(n2 == false, "nested mixed result");
    passed = passed & check(side_effect_calls == 0, "nested mixed: fully short-circuited");

    // ---------- 真实场景：null 安全检查（此前会崩溃的形态） ----------
    // 用一个可变对象模拟可能为 null 的引用；?. 与 && 组合。
    let obj: ptr<void> = null;
    const safe = obj != null && obj_has_field(obj);
    passed = passed & check(safe == false, "null guard with && does not crash");

    // 表达式直接作为 if 条件（常见用法）
    let flag = false;
    side_effect_calls = 0;
    if (flag && side_effect()) {
        passed = passed & check(false, "if (false && x) body should not run");
    }
    passed = passed & check(side_effect_calls == 0, "if (false && x) skips body and side effect");

    // ---------- 三目与短路组合 ----------
    side_effect_calls = 0;
    const t = (true && false) ? side_effect() : false;
    passed = passed & check(t == false, "ternary with short-circuit");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    flush();
    return passed == 1 ? 0 : 1;
}

function obj_has_field(o: ptr<void>): bool {
    return true;
}
