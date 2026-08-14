import { println } from "std/io";

interface Container<T> {
    get(): T;
    set(value: T): void;
}

class IntBox implements Container<int> {
    value: int;

    constructor(value: int) {
        this.value = value;
    }

    get(): int {
        return this.value;
    }

    set(value: int): void {
        this.value = value;
    }
}

class Box<T> implements Container<T> {
    value: T;

    constructor(value: T) {
        this.value = value;
    }

    get(): T {
        return this.value;
    }

    set(value: T): void {
        this.value = value;
    }
}

function read_from<T>(container: T): int where T: Container<int> {
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
    const int_box = new IntBox(42);
    const as_container: Container<int> = int_box;
    passed = passed & check(as_container.get() == 42, "concrete class implements generic iface");
    passed = passed & check(read_from(int_box) == 42, "where T: Container<int> bound");

    const box = new Box<int>(7);
    const box_container: Container<int> = box;
    passed = passed & check(box_container.get() == 7, "generic class implements generic iface");
    passed = passed & check(read_from(box) == 7, "generic class with bound");
    box.set(9);
    passed = passed & check(box_container.get() == 9, "set via interface vtable");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
