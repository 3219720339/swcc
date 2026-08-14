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

function describe(value: Option<int>): string {
    const text = match (value) {
        Some(n) => "value is " + n,
        None => "empty",
    };
    return text;
}

function area(shape: Shape): float {
    return match (shape) {
        Circle(r) => 3.14 * r * r,
        Rect(w, h) => w * h,
        Empty => 0.0,
    };
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
    const some: Option<int> = Option.Some(5);
    const none: Option<int> = Option.None;
    passed = passed & check(describe(some) == "value is 5", "match expr Some payload");
    passed = passed & check(describe(none) == "empty", "match expr None");

    const circle: Shape = Shape.Circle(2.0);
    const rect: Shape = Shape.Rect(3.0, 4.0);
    const empty: Shape = Shape.Empty;
    passed = passed & check(area(circle) == 12.56, "match expr float branch");
    passed = passed & check(area(rect) == 12.0, "match expr two bindings");
    passed = passed & check(area(empty) == 0.0, "match expr unit variant");

    const doubled = match (some) {
        Some(n) => n * 2,
        None => 0,
    };
    passed = passed & check(doubled == 10, "match expr as int expression");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
