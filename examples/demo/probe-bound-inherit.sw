import { println } from "std/io";

interface Shape {
    area(): float;
}

class BaseCircle implements Shape {
    radius: float;
    constructor(radius: float) {
        this.radius = radius;
    }
    area(): float {
        return 3.14 * this.radius * this.radius;
    }
}

// 派生类不写 implements，接口由基类继承（vtable 沿基类链收集）。
class SmallCircle extends BaseCircle {
    constructor(radius: float) {
        super(radius);
    }
}

function area_of<T>(shape: T): float where T: Shape {
    return shape.area();
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
    const base = new BaseCircle(2.0);
    const small = new SmallCircle(2.0);
    // 派生类实例作为 where T: Shape 实参（接口由基类 implements）。
    passed = passed & check(area_of(base) == 12.56, "base class satisfies bound");
    passed = passed & check(area_of(small) == 12.56, "derived class inherits bound via base");
    // 类→接口赋值沿基类链同样成立。
    const as_shape: Shape = small;
    passed = passed & check(as_shape.area() == 12.56, "derived class assignable to base interface");
    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
