function main(): int {
    const a = 10;
    const b = 5;
    const combine = (x: int) => a * x + b;
    const twice = (x: int) => x * 2;
    return combine(3) + twice(21);
}
