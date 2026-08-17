// 枚举 switch（tag 比较）+ 数字字面量 + 接口继承 + try/finally 回归。
import { println } from "std/io";

interface Shape {
    area(): float;
}
interface Named {
    name(): string;
}
interface LabeledShape extends Shape, Named {
    label(): string;
}

class Circle implements LabeledShape {
    r: float;
    constructor(radius: float) { this.r = radius; }
    area(): float { return 3.14 * this.r * this.r; }
    name(): string { return "circle"; }
    label(): string { return "C"; }
}

enum Color {
    Red,
    Green,
    Blue,
}

enum Status {
    Idle,
    Busy(int),
    Done(int),
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

    // 枚举 switch（此前指针比较静默走 default）
    const col = Color.Green;
    let r = 0;
    switch (col) {
        case Color.Red: r = 1; break;
        case Color.Green: r = 2; break;
        default: r = 9;
    }
    passed = passed & check(r == 2, "enum switch unit");

    // 枚举 switch：带字段变体（switch 只比较 tag）
    const m = Status.Busy(42);
    let r2 = 0;
    switch (m) {
        case Status.Idle: r2 = 1; break;
        case Status.Busy(0): r2 = 2; break;
        default: r2 = 9;
    }
    passed = passed & check(r2 == 2, "enum switch variant tag");

    // 数字字面量
    passed = passed & check(0xFF == 255, "hex literal");
    passed = passed & check(0b101 == 5, "binary literal");
    passed = passed & check(1_000_000 == 1000000, "underscore literal");
    passed = passed & check(1e3 == 1000.0, "sci literal");

    // float/int 字面量混合运算：float 一侧优先（不截断 float 字面量）
    passed = passed & check(2.0 * 2 == 4.0, "float*int literal");
    passed = passed & check(2 * 2.0 == 4.0, "int*float literal");
    passed = passed & check(10.0 / 4 == 2.5, "float/int division");
    passed = passed & check(2.0 ** 2 == 4.0, "float**int literal");
    passed = passed & check(5.5 % 2 == 1.5, "float%int literal");
    let f: float = 3.14;
    passed = passed & check(f > 3 && f < 4, "float cmp int literal");
    passed = passed & check(10 / 4 == 2, "int division stays int");

    // 接口继承
    const c: LabeledShape = new Circle(2.0);
    passed = passed & check(c.area() == 12.56, "iface inherit area");
    const n: Named = new Circle(2.0);
    passed = passed & check(n.name() == "circle", "iface inherit name");

    // try/finally 无 catch
    let cleaned = 0;
    try {
        cleaned = 1;
    } finally {
        cleaned = cleaned + 10;
    }
    passed = passed & check(cleaned == 11, "try-finally");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
