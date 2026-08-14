import { expect_eq } from "std/test";

@test function int_eq(): int {
    expect_eq(5, 5);
    return 0;
}

@test function string_eq(): int {
    expect_eq("ab", "ab");
    return 0;
}

struct Point {
    x: int;
    y: int;
}

@test function struct_eq(): int {
    const p1: Point = { x: 1, y: 2 };
    const p2: Point = { x: 1, y: 2 };
    expect_eq(p1, p2);
    return 0;
}
