import { println, flush } from "std/io";

// 语言完整度探针：浮点全局变量 + 对象字面量目标类型传播 + 对象解构。
function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

// 浮点全局（此前崩溃：全局按 i64 槽存 float，fmul 收到 i64 操作数）
const PI: float = 3.14159;
const TAU: float = 6.28318;
const COUNT: int = 42;
let SCALE: float = 1.0;

struct Point {
    x: int;
    y: int;
}

struct Inner {
    v: int;
}

struct Outer {
    name: string;
    inner: Inner;
}

function area(p: Point): int {
    return p.x * p.y;
}

function make_point(): Point {
    return { x: 7, y: 6 };  // return 字面量传播
}

function show(o: Outer): string {
    return o.name + ":" + o.inner.v;
}

function main(): int {
    let passed = 1;

    // ---------- 浮点全局变量 ----------
    const r = 2.0;
    const circle = PI * r * r;
    passed = passed & check(circle > 12.0 && circle < 13.0, "float global const");
    passed = passed & check(COUNT == 42, "int global beside float");
    passed = passed & check(TAU > 6.2 && TAU < 6.3, "second float global");
    SCALE = 2.5;  // mutable float 全局赋值
    const scaled = 10.0 * SCALE * 0.5;
    passed = passed & check(scaled == 12.5, "mutable float global assign");
    passed = passed & check(SCALE == 2.5, "float global read after assign");

    // ---------- 对象字面量：实参 ----------
    const a = area({ x: 3, y: 4 });
    passed = passed & check(a == 12, "object literal as call arg");

    // ---------- 对象字面量：return ----------
    const b = area(make_point());
    passed = passed & check(b == 42, "object literal as return value");

    // ---------- 对象字面量：赋值 ----------
    let p: Point = { x: 1, y: 2 };
    p = { x: 5, y: 6 };
    passed = passed & check(area(p) == 30, "object literal assignment");

    // ---------- 对象字面量：数组元素 ----------
    const pts: Point[] = [{ x: 10, y: 20 }];
    passed = passed & check(area(pts[0]) == 200, "object literal in typed array");

    // ---------- 对象字面量：嵌套字段 ----------
    const o: Outer = { name: "sw", inner: { v: 7 } };
    passed = passed & check(show(o) == "sw:7", "nested object literal field");

    // 嵌套字面量作实参
    const r2 = show({ name: "x", inner: { v: 9 } });
    passed = passed & check(r2 == "x:9", "nested object literal as arg");

    // ---------- 对象解构 ----------
    const { x, y } = p;
    passed = passed & check(x == 5 && y == 6, "object destructuring from variable");

    const [da, db] = [10, 20];
    passed = passed & check(da == 10 && db == 20, "array destructuring");

    // 解构嵌套 struct 字段
    const { inner } = o;
    passed = passed & check(inner.v == 7, "destructure nested struct field");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    flush();
    return passed == 1 ? 0 : 1;
}
