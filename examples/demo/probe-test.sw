import { println } from "std/io";
import { Result } from "std/result";
import { assert, expect_eq, expect_true, expect_false, fail } from "std/test";

function add(a: int, b: int): int {
    return a + b;
}
@test
function test_add(): int {
    expect_eq(add(1, 2), 3);
    assert(add(0, 0) == 0);
    assert(add(-1, 1) == 0, "负数相加");
    return 0;
}

@test function test_string(): int {
    const text = "hello";
    expect_eq(text.length, 5);
    expect_eq(text[1], 'e');
    expect_eq(text.to_upper(), "HELLO");
    expect_true(text.starts_with("he"));
    expect_false(text.starts_with("wo"));
    return 0;
}

@test function test_match(): int {
    const value: Result<int, string> = Result.Ok(7);
    match (value) {
        Ok(v) => {
            expect_eq(v, 7);
            return 0;
        }
        Err(_) => {
            return 1;
        }
    }
}

struct Point {
    x: int;
    y: int;
}

@test function test_array_compare(): int {
    const a = [1, 2, 3];
    const b = [1, 2, 3];
    const c = [1, 2, 4];
    expect_eq(a, b);
    expect_eq(["x", "y"], ["x", "y"]);
    expect_eq([1.5, 2.5], [1.5, 2.5]);
    if (a.length == 3 && b[2] == 3 && c[2] == 4) {
        return 0;
    }
    return 1;
}

@test function test_struct_compare(): int {
    const p1: Point = { x: 1, y: 2 };
    const p2: Point = { x: 1, y: 2 };
    const p3: Point = { x: 1, y: 3 };
    expect_eq(p1, p2);
    if (p3.y == 3 && p1.x == 1) {
        return 0;
    }
    return 1;
}
