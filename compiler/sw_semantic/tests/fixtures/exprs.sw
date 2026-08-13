struct Point {
    x: int;
    y: float;
}

function make(): Point {
    const p: Point = { x: 2, y: 3.5 };
    return p;
}

function sum_x(p: Point): int {
    return p.x + 1;
}

function main(): int {
    let i = 0;
    const old = i++;
    const fresh = ++i;
    let a = 1;
    const assigned = (a = 5);
    const p = make();
    const total = sum_x(p);
    const fy = p.y;
    const fsum = fy + 1.5;
    const power = 2 ** 10;
    const fpower = 2.0 ** 3.0;
    const rem = 7.5 % 2.0;
    if (
        old == 0 &&
        fresh == 2 &&
        assigned == 5 &&
        total == 3 &&
        fsum == 5.0 &&
        power == 1024 &&
        fpower == 8.0 &&
        rem == 1.5
    ) {
        return 0;
    }
    return 1;
}
