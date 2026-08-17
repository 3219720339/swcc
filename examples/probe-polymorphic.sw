// 多态数组（接口数组 + struct 数组 return 字面量）。
// 接口数组：`Shape[] = [Circle, Square]` 元素按 is_assignable 校验，
// 支持异构 class 实现同一接口，经接口 vtable 派发。
// struct 数组 return：`return [{...}]` 对象元素按返回类型传播目标类型。
import { println } from "std/io";

interface Shape {
    area(): float;
}

class Circle implements Shape {
    r: float;
    constructor(radius: float) { this.r = radius; }
    area(): float { return 3.14 * this.r * this.r; }
}

class Square implements Shape {
    s: float;
    constructor(side: float) { this.s = side; }
    area(): float { return this.s * this.s; }
}

struct Point {
    x: int;
    y: int;
}

function make_points(): Point[] {
    return [{ x: 1, y: 2 }, { x: 3, y: 4 }];
}

function total_area(shapes: Shape[]): float {
    let sum = 0.0;
    for (const s of shapes) {
        sum = sum + s.area();
    }
    return sum;
}

function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

// 浮点累加（3.14 + 4.0）与字面量 7.14 存在 IEEE-754 表示误差，用容差比较。
function feq(a: float, b: float): bool {
    let d = a - b;
    if (d < 0.0) { d = -d; }
    return d < 0.0001;
}

function main(): int {
    let passed = 1;
    // 接口多态数组：异构 class 实现
    const shapes: Shape[] = [new Circle(1.0), new Square(2.0)];
    passed = passed & check(shapes.length == 2, "interface array length");
    passed = passed & check(shapes[0].area() == 3.14, "interface array dispatch circle");
    passed = passed & check(shapes[1].area() == 4.0, "interface array dispatch square");
    let total = 0.0;
    for (const s of shapes) {
        total = total + s.area();
    }
    passed = passed & check(feq(total, 7.14), "interface array for-of dispatch");

    // 接口数组类型不兼容报错（check 阶段，不运行）
    // const bad: Shape[] = [new Circle(1.0), 42]; // 应报"元素类型不兼容"

    // struct 数组 return 字面量
    const pts = make_points();
    passed = passed & check(pts.length == 2, "struct array return length");
    passed = passed & check(pts[0].x == 1 && pts[0].y == 2, "struct array return element 0");
    passed = passed & check(pts[1].x == 3 && pts[1].y == 4, "struct array return element 1");

    // 接口数组作为函数参数
    passed = passed & check(feq(total_area(shapes), 7.14), "interface array as param");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
