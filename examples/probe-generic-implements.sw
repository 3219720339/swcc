import { println } from "std/io";

interface Container<T> {
    get(): T;
    set(value: T): void;
}

interface Shape {
    area(): float;
}

// A: 泛型类 implements 具体类型实参
class Fixed<T> implements Container<int> {
    value: int;
    constructor(value: int) { this.value = value; }
    get(): int { return this.value; }
    set(value: int): void { this.value = value; }
}

// B: 多类型参数
class Pair<A, B> implements Container<A> {
    a: A;
    b: B;
    constructor(a: A, b: B) { this.a = a; this.b = b; }
    get(): A { return this.a; }
    set(value: A): void { this.a = value; }
}

// C: 双接口（一个泛型一个非泛型）
class Dual<T> implements Container<T>, Shape {
    value: T;
    constructor(value: T) { this.value = value; }
    get(): T { return this.value; }
    set(value: T): void { this.value = value; }
    area(): float { return 3.14; }
}

// D: 泛型继承 + 接口（基类实参用自身类型参数）
class Box<T> implements Container<T> {
    value: T;
    constructor(value: T) { this.value = value; }
    get(): T { return this.value; }
    set(value: T): void { this.value = value; }
}

class SubBox<T> extends Box<T> {
    extra: int;
    constructor(value: T, extra: int) {
        super(value);
        this.extra = extra;
    }
}

// D2: 二级泛型继承（SubSubBox<T> extends SubBox<T> extends Box<T>）
class SubSubBox<T> extends SubBox<T> {
    more: int;
    constructor(value: T, extra: int, more: int) {
        super(value, extra);
        this.more = more;
    }
}

// D3: 泛型类 extends 具体实参基类 + 自身实现接口
class IntSub<T> extends Box<int> implements Shape {
    extra2: T;
    constructor(value: int, extra2: T) {
        super(value);
        this.extra2 = extra2;
    }
    area(): float { return 1.0; }
}

// E: where T: Container<U> 反向推导，实参为泛型类实例
function read_via<T, U>(container: T): U where T: Container<U> {
    return container.get();
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

    const fixed: Container<int> = new Fixed<string>(42);
    passed = passed & check(fixed.get() == 42, "A: generic class implements Container<int>");

    const pair: Container<int> = new Pair<int, string>(5, "hi");
    passed = passed & check(pair.get() == 5, "B: multi type params");
    pair.set(9);
    passed = passed & check(pair.get() == 9, "B: set via vtable");

    const dual: Container<string> = new Dual<string>("s");
    const shape: Shape = new Dual<float>(1.5);
    passed = passed & check(dual.get() == "s", "C: generic iface dispatch");
    passed = passed & check(shape.area() == 3.14, "C: non-generic iface dispatch");

    const sub: Container<int> = new SubBox<int>(3, 99);
    passed = passed & check(sub.get() == 3, "D: derived generic class inherits iface");
    const sub2 = new SubBox<int>(3, 99);
    passed = passed & check(sub2.extra == 99, "D: derived own field");
    const as_box: Box<int> = new SubBox<int>(7, 1);
    passed = passed & check(as_box.get() == 7, "D: SubBox<int> assignable to Box<int>");

    const ss: Container<int> = new SubSubBox<int>(1, 2, 3);
    passed = passed & check(ss.get() == 1, "D2: two-level generic inheritance");
    const ss2 = new SubSubBox<int>(1, 2, 3);
    passed = passed & check(ss2.more == 3, "D2: two-level own field");

    const is_: Container<int> = new IntSub<string>(8, "x");
    passed = passed & check(is_.get() == 8, "D3: extends Box<int> + own implements");
    const is_shape: Shape = new IntSub<string>(8, "x");
    passed = passed & check(is_shape.area() == 1.0, "D3: Shape dispatch");

    const box = new Box<int>(7);
    const v: int = read_via(box);
    passed = passed & check(v == 7, "E: derive U from generic-class instance");

    // F: 嵌套泛型（Box<Box<int>> / Container<Box<int>>）
    const nb = new Box<Box<int>>(new Box<int>(5));
    const nc: Container<Box<int>> = nb;
    passed = passed & check(nc.get().get() == 5, "F: nested generic + vtable");
    nc.get().set(9);
    passed = passed & check(nc.get().get() == 9, "F: nested set via vtable");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
