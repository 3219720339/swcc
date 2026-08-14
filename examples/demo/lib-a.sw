// lib-a.sw：与 lib-b.sw 相互调用（循环 import）。
import { helper_b } from "./lib-b";

export function helper_a(): string {
    return "from-a";
}

export function combined_a(): string {
    return "a+" + helper_b();
}

export function double_a(value: int): int {
    return value * 2;
}
