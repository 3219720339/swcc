// 数组 slice/sort_by + 方法引用作闭包 + 可选调用 f?.()。
import { println } from "std/io";

class Counter {
    n: int;
    constructor(v: int) { this.n = v; }
    inc(): int { this.n = this.n + 1; return this.n; }
    add(x: int): int { this.n = this.n + x; return this.n; }
}

struct Point {
    x: int;
    y: int;
}

function apply(f: () => int): int { return f(); }

function check(c: bool, label: string): int {
    if (c) { println(`[ok] ${label}`); return 1; }
    println(`[FAIL] ${label}`);
    return 0;
}

function main(): int {
    let passed = 1;
    const nums = [3, 1, 4, 1, 5, 9, 2, 6];

    // arr.slice(start[, end])：负索引、省略 end、非变异
    const s1 = nums.slice(2, 5);
    passed = passed & check(s1.length == 3 && s1[0] == 4 && s1[2] == 5, "slice range");
    passed = passed & check(nums.slice(-3).length == 3, "slice negative");
    passed = passed & check(nums.slice(5, 2).length == 0, "slice empty range");
    passed = passed & check(nums.length == 8, "slice non-mutating");

    // arr.sort_by((a, b) => bool)：int/string/struct，升/降序
    const asc = nums.sort_by((a: int, b: int): bool => a < b);
    passed = passed & check(asc[0] == 1 && asc[7] == 9, "sort_by int");
    const words = ["banana", "apple"];
    passed = passed & check(words.sort_by((a: string, b: string): bool => a < b)[0] == "apple", "sort_by string");
    const pts: Point[] = [{ x: 3, y: 1 }, { x: 1, y: 2 }];
    passed = passed & check(pts.sort_by((a: Point, b: Point): bool => a.x < b.x)[0].x == 1, "sort_by struct");
    passed = passed & check(nums.sort_by((a: int, b: int): bool => a > b)[0] == 9, "sort_by descending");

    // 方法引用作闭包（绑定 this）
    const c = new Counter(1);
    const inc = c.inc;
    passed = passed & check(inc() == 2 && inc() == 3, "method ref bound this");
    passed = passed & check(c.add(10) == 13, "method ref with arg");
    passed = passed & check(apply(c.inc) == 14, "method ref to hof");

    // 可选调用 f?.()
    let fn: ((int) => int)? = (x: int): int => x + 1;
    passed = passed & check(fn?.(5) == 6, "optional call non-null");
    fn = null;
    passed = passed & check((fn?.(5) ?? 0) == 0, "optional call null");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
