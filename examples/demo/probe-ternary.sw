function pick(flag: bool): int {
    return flag ? 42 : 0;
}

function main(): int {
    const a = pick(true);
    const b = pick(false);
    return a + b + (true ? 1 : 99);
}
