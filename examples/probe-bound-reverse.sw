import { println } from "std/io";

interface Container<T> {
    get(): T;
    set(value: T): void;
}

interface Shape {
    area(): float;
}

class IntBox implements Container<int> {
    value: int;
    constructor(value: int) {
        this.value = value;
    }
    get(): int { return this.value; }
    set(value: int): void { this.value = value; }
}

class StrBox implements Container<string> {
    value: string;
    constructor(value: string) {
        this.value = value;
    }
    get(): string { return this.value; }
    set(value: string): void { this.value = value; }
}

// 约束接口实参是另一个类型参数（where T: Container<U>）——需从实参类实现
// 的同模板接口具体实例反向推导 U，并经接口 vtable 派发。
function read_via<T, U>(container: T): U where T: Container<U> {
    return container.get();
}

function write_via<T, U>(container: T, value: U): void where T: Container<U> {
    container.set(value);
}

class Circle implements Shape {
    radius: float;
    constructor(radius: float) { this.radius = radius; }
    area(): float { return 3.14 * this.radius * this.radius; }
}

// 非泛型接口约束（where T: Shape）回归：确保不受影响。
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
    const ib = new IntBox(42);
    const sb = new StrBox("hello");

    // where T: Container<U>：U 从 IntBox implements Container<int> 反推为 int。
    const iv: int = read_via(ib);
    passed = passed & check(iv == 42, "derive U=int via bound, vtable get");
    const sv: string = read_via(sb);
    passed = passed & check(sv == "hello", "derive U=string via bound");

    // set 经 vtable 调用，再读回。
    write_via(ib, 7);
    passed = passed & check(read_via(ib) == 7, "set via bound vtable");

    // 非泛型接口约束不回归。
    const c = new Circle(2.0);
    passed = passed & check(area_of(c) == 12.56, "non-generic bound still works");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
