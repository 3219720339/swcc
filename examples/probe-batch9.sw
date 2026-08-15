import { println, flush } from "std/io";
import { char_at, left, right, starts_with_any, ends_with_any, edit_distance } from "std/string";
import { factorial, is_prime, next_prime, percent, round_to, lerp } from "std/math";
import {
    flatten_string,
    first_int,
    last_int,
    first_float,
    last_float,
    first_string,
    last_string,
    avg_float,
} from "std/array";
import { map_new, map_set, map_get, map_len, map_merge, map_from_arrays } from "std/map";
import { console_clear_line, console_title } from "std/console";
import { memory_usage_kb } from "std/os";

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

    // ---------- std/string 常用 ----------
    passed = passed & check(char_at("你好a", 1) == "好", "char_at UTF-8");
    passed = passed & check(char_at("abc", 5) == "", "char_at out of range");
    passed = passed & check(left("hello", 2) == "he", "left");
    passed = passed & check(left("hi", 10) == "hi", "left clamp");
    passed = passed & check(right("hello", 2) == "lo", "right");
    passed = passed & check(right("你好a", 1) == "a", "right UTF-8");
    passed = passed & check(
        starts_with_any("http://x", ["http://", "https://"]),
        "starts_with_any true"
    );
    passed = passed & check(!starts_with_any("ftp://x", ["http://"]), "starts_with_any false");
    passed = passed & check(ends_with_any("file.txt", [".txt", ".md"]), "ends_with_any true");
    passed = passed & check(!ends_with_any("file.md", [".txt"]), "ends_with_any false");
    passed = passed & check(edit_distance("kitten", "sitting") == 3, "edit_distance");
    passed = passed & check(edit_distance("abc", "abc") == 0, "edit_distance equal");
    passed = passed & check(edit_distance("", "abc") == 3, "edit_distance empty");

    // ---------- std/math 基础 ----------
    passed = passed & check(factorial(5) == 120, "factorial");
    passed = passed & check(factorial(0) == 1, "factorial 0");
    passed = passed & check(factorial(-1) == 0 && factorial(21) == 0, "factorial guard");
    passed = passed & check(is_prime(2) && is_prime(3) && is_prime(97), "is_prime true");
    passed = passed & check(!is_prime(4) && !is_prime(1) && !is_prime(99), "is_prime false");
    passed = passed & check(next_prime(10) == 11 && next_prime(1) == 2 && next_prime(2) == 3, "next_prime");
    passed = passed & check(percent(25, 200) == 12.5, "percent");
    passed = passed & check(percent(50, 0) == 0.0, "percent zero total");
    passed = passed & check(round_to(3.14159, 2) == 3.14, "round_to");
    passed = passed & check(round_to(2.5, 0) == 3.0, "round_to 0 digits");
    passed = passed & check(lerp(0.0, 10.0, 0.5) == 5.0, "lerp");
    passed = passed & check(lerp(1.0, 3.0, 0.25) == 1.5, "lerp quarter");

    // ---------- std/array 取值/统计 ----------
    const flat = flatten_string([["a", "b"], ["c"]]);
    passed = passed & check(flat.length == 3 && flat[2] == "c", "flatten_string");
    passed = passed & check(first_int([1, 2], 0) == 1 && first_int([], 9) == 9, "first_int");
    passed = passed & check(last_int([1, 2], 0) == 2 && last_int([], 9) == 9, "last_int");
    passed = passed & check(first_float([1.5], 0.0) == 1.5, "first_float");
    passed = passed & check(last_float([], 2.5) == 2.5, "last_float fallback");
    passed = passed & check(first_string(["a"], "x") == "a", "first_string");
    passed = passed & check(last_string(["a", "b"], "x") == "b", "last_string");
    passed = passed & check(avg_float([1.0, 2.0, 3.0]) == 2.0, "avg_float");
    passed = passed & check(avg_float([]) == 0.0, "avg_float empty");

    // ---------- std/map 构建/合并 ----------
    const ma = map_new();
    map_set(ma, "x", "1");
    map_set(ma, "y", "2");
    const mb = map_new();
    map_set(mb, "y", "3");
    map_set(mb, "z", "4");
    const merged = map_merge(ma, mb);
    passed = passed & check(map_len(merged) == 3, "map_merge len");
    passed = passed & check((map_get(merged, "y") ?? "") == "3", "map_merge override");
    passed = passed & check((map_get(merged, "z") ?? "") == "4", "map_merge add");
    const from_arrays = map_from_arrays(["a", "b"], ["1", "2"]);
    passed = passed & check(map_len(from_arrays) == 2 && (map_get(from_arrays, "a") ?? "") == "1", "map_from_arrays");
    const short_vals = map_from_arrays(["a", "b"], ["1"]);
    passed = passed & check(map_len(short_vals) == 1, "map_from_arrays shorter");

    // ---------- io/console/os 工具 ----------
    flush();
    console_clear_line();
    console_title("sw probe-batch9");
    passed = passed & check(memory_usage_kb() > 0, "memory_usage_kb > 0");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
