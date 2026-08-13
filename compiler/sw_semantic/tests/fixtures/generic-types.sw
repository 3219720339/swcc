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
    const p: Pair<int> = { first: 1, second: 2 };
    const sum = p.first + p.second;
    const box: Box<int> = new Box<int>(42);
    return sum + box.get();
}
