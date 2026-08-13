struct Point {
    x: float;
    y: float;
}

function main(): int {
    const point: Point = { x: 1.0, y: 2.0 };
    const sum = point.x + point.y;
    if (sum > 0.0) {
        return 42;
    }
    return 0;
}
