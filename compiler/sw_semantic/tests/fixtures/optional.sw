class Node {
    value: int;
    constructor(v: int) {
        this.value = v;
    }
}

function main(): int {
    const n: Node? = new Node(7);
    const a = n?.value ?? 0;
    const none: Node? = null;
    const b = none?.value ?? 0;
    return a + b;
}
