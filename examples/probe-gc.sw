import { println } from "std/io";
import { map_new, map_set, map_get } from "std/map";

struct Point {
    x: int;
    y: float;
    name: string;
}

class Item {
    data: string;

    constructor(d: string) {
        this.data = d;
    }
}

function risky(flag: bool): int {
    if (flag) {
        throw "boom";
    }
    return 42;
}

function check(prev: int, cond: bool, label: string): int {
    let state = "FAIL";
    if (cond) {
        state = "ok";
    }
    println(`[${state}] ${label}`);
    if (cond) {
        return prev;
    }
    return 0;
}

function main(): int {
    let ok = 1;

    // 1) 大量分配强制多次 GC 回收，验证字符串/数组/类/struct/map 在回收后仍完整
    let check1 = 0;
    let i = 0;
    while (i < 80000) {
        const s = `v${i}`;
        const arr: int[] = [i, i + 1, i + 2];
        const p: Point = { x: i, y: 1.5, name: s };
        const item = new Item(s);
        const m = map_new();
        map_set(m, "k", s);
        if (i == 79999) {
            let good = 1;
            if (s != "v79999") {
                good = 0;
            }
            if (arr[2] != i + 2) {
                good = 0;
            }
            if (p.x != i) {
                good = 0;
            }
            if (p.name != s) {
                good = 0;
            }
            if (item.data != s) {
                good = 0;
            }
            if ((map_get(m, "k") ?? "") != s) {
                good = 0;
            }
            check1 = good;
        }
        i = i + 1;
    }
    ok = check(ok, check1 == 1, "gc_pressure_integrity");

    // 2) 高频抛异常/捕获（帧与异常对象走 GC，验证不泄漏、不崩溃）
    let caught = 0;
    let j = 0;
    while (j < 30000) {
        try {
            risky(true);
        } catch (e: string) {
            caught = caught + 1;
        }
        j = j + 1;
    }
    ok = check(ok, caught == 30000, "gc_exc_loop");

    // 3) 异常捕获后继续分配（异常对象应可被回收，不影响后续分配）
    let k = 0;
    while (k < 20000) {
        const s = `a${k}`;
        let hit = 0;
        try {
            risky(true);
        } catch (e: string) {
            if (e == "boom") {
                hit = 1;
            }
        }
        if (k == 19999) {
            let good = 1;
            if (hit != 1) {
                good = 0;
            }
            if (s != "a19999") {
                good = 0;
            }
            ok = check(ok, good == 1, "gc_exc_then_alloc");
        }
        k = k + 1;
    }

    println(`final=${ok == 1 ? "PASS" : "FAIL"}`);
    return ok == 1 ? 0 : 1;
}
