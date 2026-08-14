import { println } from "std/io";

interface Shape {
    area(): float;
    label(): string;
}

class Circle implements Shape {
    radius: float;

    constructor(radius: float) {
        this.radius = radius;
    }

    area(): float {
        return 3.14 * this.radius * this.radius;
    }

    label(): string {
        return "circle";
    }
}

class Box<T> implements Shape {
    value: T;

    constructor(value: T) {
        this.value = value;
    }

    area(): float {
        return 3.25;
    }

    label(): string {
        return "box";
    }
}

function area_of<T>(shape: T): float where T: Shape {
    return shape.area();
}

function describe<T>(shape: T): string where T: Shape {
    return shape.label();
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
    const circle = new Circle(2.0);
    passed = passed & check(area_of(circle) == 12.56, "generic bound method");
    passed = passed & check(describe(circle) == "circle", "generic bound second method");

    const box = new Box<int>(7);
    const as_shape: Shape = box;
    passed = passed & check(area_of(box) == 3.25, "generic class implements interface");
    passed = passed & check(as_shape.label() == "box", "generic class vtable dispatch");
    passed = passed & check(box.value == 7, "generic class field type replaced");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
