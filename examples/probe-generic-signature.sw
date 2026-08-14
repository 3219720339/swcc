import { println } from "std/io";

interface Container<T> {
    get(): T;
    set(value: T): void;
}

class Box<T> implements Container<T> {
    value: T;
    constructor(value: T) { this.value = value; }
    get(): T { return this.value; }
    set(v: T): void { this.value = v; }
}

struct Pair<A, B> {
    first: A;
    second: B;
}

// 1: 泛型函数返回泛型类
function make<T>(x: T): Box<T> {
    return new Box<T>(x);
}

// 2: 泛型函数参数是泛型类
function read_box<T>(b: Box<T>): T {
    return b.get();
}

// 3: 嵌套返回类型 Box<Box<T>>
function make2<T>(x: T): Box<Box<T>> {
    return new Box<Box<T>>(new Box<T>(x));
}

// 4: 返回类型是泛型接口实例 Container<T>
function wrap<T>(x: T): Container<T> {
    return new Box<T>(x);
}

// 5: 泛型函数返回泛型 struct
function make_pair<A, B>(a: A, b: B): Pair<A, B> {
    const p: Pair<A, B> = { first: a, second: b };
    return p;
}

// 6: T 从两个实参推导
function both<T>(a: Box<T>, b: Box<T>): T {
    a.set(b.get());
    return a.get();
}

// 7: 泛型函数嵌套调用泛型函数
function nest<T>(x: T): Box<T> {
    return make(x);
}

// 8: 泛型类方法返回泛型类（this 上下文）
class Factory<T> {
    value: T;
    constructor(value: T) { this.value = value; }
    make_box(): Box<T> {
        return new Box<T>(this.value);
    }
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

    const b = make(42);
    passed = passed & check(b.get() == 42, "1a: make(int) returns Box<int>");
    const s = make("hi");
    passed = passed & check(s.get() == "hi", "1b: make(string) returns Box<string>");
    b.set(99);
    passed = passed & check(b.get() == 99, "1c: mutate via instance");

    passed = passed & check(read_box(b) == 99, "2: generic class param");

    const nb = make2(7);
    passed = passed & check(nb.get().get() == 7, "3: nested Box<Box<int>> return");

    const c: Container<int> = wrap(5);
    passed = passed & check(c.get() == 5, "4a: return Container<int>");
    const cs: Container<string> = wrap("x");
    passed = passed & check(cs.get() == "x", "4b: return Container<string>");

    const p = make_pair(1, "one");
    passed = passed & check(p.first == 1 && p.second == "one", "5: generic struct return");

    const b1 = make(1);
    const b2 = make(9);
    passed = passed & check(both(b1, b2) == 9, "6: T from two args");

    const n = nest(42);
    passed = passed & check(n.get() == 42, "7: generic calls generic");

    const f = new Factory<string>("fv");
    passed = passed & check(f.make_box().get() == "fv", "8: generic class method returns generic class");
    const fi = new Factory<int>(7);
    passed = passed & check(fi.make_box().get() == 7, "8b: factory int");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
