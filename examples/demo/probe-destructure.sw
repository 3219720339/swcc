import { println } from "std/io";

struct Point {
    x: int;
    y: int;
}

function pair(): int[] {
    return [3, 4];
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
    const [a, b] = [1, 2];
    passed = passed & check(a == 1 && b == 2, "array destructure literal");

    const [c, d] = pair();
    passed = passed & check(c == 3 && d == 4, "array destructure function call");

    const p: Point = { x: 7, y: 8 };
    const { x, y } = p;
    passed = passed & check(x == 7 && y == 8, "object destructure");

    const { x: renamed } = p;
    passed = passed & check(renamed == 7, "object destructure rename");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
