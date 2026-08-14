// @test 断言辅助：失败时抛异常，由 swc test 的 runner 捕获并记为失败。
// 用法：
//   import { assert, expect_eq, expect_true, expect_false, fail } from "std/test";
//   @test function demo(): void {
//       assert(1 + 1 == 2);
//       assert(1 + 1 == 2, "算术错误");
//       expect_eq(2 + 3, 5);
//       expect_eq("hello", "hello");
//       expect_eq(1.5, 1.5);
//       expect_true(true);
//       expect_false(false);
//   }

export function assert(condition: bool): void {
    if (!condition) {
        throw "assertion failed";
    }
}

export function assert(condition: bool, message: string): void {
    if (!condition) {
        throw message;
    }
}

// 标量 / struct / 可空值：按值比较（struct 为逐字段值比较）。
export function expect_eq<T>(actual: T, expected: T): void {
    if (actual != expected) {
        throw "values not equal";
    }
}

// 数组按长度 + 逐元素比较。
export function expect_eq(actual: int[], expected: int[]): void {
    if (actual.length != expected.length) {
        throw "array length mismatch: expected " + expected.length + ", got " + actual.length;
    }
    for (let i = 0; i < actual.length; i++) {
        if (actual[i] != expected[i]) {
            throw "arrays differ at index " + i;
        }
    }
}

export function expect_eq(actual: float[], expected: float[]): void {
    if (actual.length != expected.length) {
        throw "array length mismatch: expected " + expected.length + ", got " + actual.length;
    }
    for (let i = 0; i < actual.length; i++) {
        if (actual[i] != expected[i]) {
            throw "arrays differ at index " + i;
        }
    }
}

export function expect_eq(actual: string[], expected: string[]): void {
    if (actual.length != expected.length) {
        throw "array length mismatch: expected " + expected.length + ", got " + actual.length;
    }
    for (let i = 0; i < actual.length; i++) {
        if (actual[i] != expected[i]) {
            throw "arrays differ at index " + i;
        }
    }
}

export function expect_true(actual: bool): void {
    if (!actual) {
        throw "expected true, got false";
    }
}

export function expect_false(actual: bool): void {
    if (actual) {
        throw "expected false, got true";
    }
}

export function fail(message: string): void {
    throw message;
}
