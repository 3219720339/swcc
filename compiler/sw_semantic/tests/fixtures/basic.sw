function add(a: int, b: int): int {
    return a + b;
}

function main(): int {
    let total = 0;
    for (let i = 0; i < 10; i++) {
        total += i;
    }
    if (total > 40) {
        total = 42;
    } else {
        total = add(total, 0);
    }
    return total;
}
