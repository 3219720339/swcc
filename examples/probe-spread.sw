import { println } from "std/io";

struct Point {
    x: int;
    y: int;
}

function add3(a: int, b: int, c: int): int {
    return a + b + c;
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
    const a = [1, 2];
    const b = [3, 4];
    const merged = [...a, 99, ...b];
    passed = passed & check(merged.length == 5, "spread length");
    passed = passed & check(merged[0] == 1 && merged[2] == 99 && merged[4] == 4, "spread values");

    let flag = true;
    flag &&= false;
    passed = passed & check(flag == false, "logical and assign");
    let fallback = false;
    fallback ||= true;
    passed = passed & check(fallback == true, "logical or assign");

    const origin: Point = { x: 1, y: 2 };
    const moved: Point = { ...origin, y: 9 };
    passed = passed & check(moved.x == 1 && moved.y == 9, "struct spread with override");
    const copied: Point = { ...origin };
    passed = passed & check(copied.x == 1 && copied.y == 2, "struct spread copy");

    passed = passed & check(add3(...[1, 2, 3]) == 6, "call spread literal");
    passed = passed & check(add3(1, ...[2, 3]) == 6, "call spread mixed");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
