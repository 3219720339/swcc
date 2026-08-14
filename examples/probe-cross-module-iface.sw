import { println } from "std/io";
import { Box, Circle, Container, Shape, SubBox } from "./lib-cross-iface";

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

    // 跨模块泛型类 implements 泛型接口：vtable 在定义模块生成、使用模块 Import。
    const box = new Box<int>(11);
    const c: Container<int> = box;
    passed = passed & check(c.get() == 11, "cross-module generic class vtable get");
    c.set(22);
    passed = passed & check(c.get() == 22, "cross-module generic class vtable set");

    // 跨模块非泛型类 implements 非泛型接口。
    const circle = new Circle(2.0);
    const s: Shape = circle;
    passed = passed & check(s.area() == 12.56, "cross-module non-generic vtable");

    // 跨模块泛型继承 + 接口（SubBox<int> extends Box<int> implements Container<int>）。
    const sub: Container<int> = new SubBox<int>(5, 99);
    passed = passed & check(sub.get() == 5, "cross-module generic inheritance vtable");
    const sub2 = new SubBox<int>(5, 99);
    passed = passed & check(sub2.extra == 99, "cross-module derived field");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
