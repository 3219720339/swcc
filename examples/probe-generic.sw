import { println } from "std/io";

struct Pair<T> {
    first: T;
    second: T;
}

class Box<T> {
    value: T;
    constructor(v: T) {
        this.value = v;
    }
    get(): T {
        return this.value;
    }
}

function main(): int {
    const p: Pair<int> = { first: 3, second: 4 };
    const box: Box<int> = new Box<int>(55);
    println(`sum=${p.first + p.second} got=${box.get()}`);
    return 0;
}
