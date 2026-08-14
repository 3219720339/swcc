// lib-b.sw：与 lib-a.sw 相互调用（循环 import）。
import { helper_a, double_a } from "./lib-a";

export function helper_b(): string {
    return "from-b";
}

export function combined_b(): string {
    return "b+" + helper_a();
}

export function transform(value: int): int {
    return double_a(value) + 1;
}
