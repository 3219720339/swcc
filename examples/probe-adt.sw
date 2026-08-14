import { println } from "std/io";

enum Option<T> {
    Some(T),
    None,
}

enum Shape {
    Circle(float),
    Rect(float, float),
    Empty,
}

function unwrap_or(option: Option<int>, fallback: int): int {
    match (option) {
        Some(value) => {
            return value;
        }
        None => {
            return fallback;
        }
    }
}

function area(shape: Shape): float {
    match (shape) {
        Circle(radius) => {
            return 3.14 * radius * radius;
        }
        Rect(width, height) => {
            return width * height;
        }
        Empty => {
            return 0.0;
        }
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
    const some: Option<int> = Option.Some(42);
    const none: Option<int> = Option.None;
    passed = passed & check(unwrap_or(some, 0) == 42, "match Some payload");
    passed = passed & check(unwrap_or(none, 7) == 7, "match None fallback");

    const circle: Shape = Shape.Circle(2.0);
    const rect: Shape = Shape.Rect(3.0, 4.0);
    const empty: Shape = Shape.Empty;
    passed = passed & check(area(circle) == 12.56, "match variant with float field");
    passed = passed & check(area(rect) == 12.0, "match variant with two fields");
    passed = passed & check(area(empty) == 0.0, "match unit variant");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
