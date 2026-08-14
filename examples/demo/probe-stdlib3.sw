import { println } from "std/io";
import {
    sort_int,
    sort_float,
    sort_string,
    reverse_int,
    reverse_float,
    reverse_string,
    min_int,
    max_int,
    sum_int,
    min_float,
    max_float,
    sum_float,
    unique_string,
} from "std/array";
import {
    map_new,
    map_set,
    map_get,
    map_has,
    map_remove,
    map_len,
    map_keys,
} from "std/map";

function check(prev: int, cond: bool, label: string): int {
    let state = "FAIL";
    if (cond) {
        state = "ok";
    }
    println(`[${state}] ${label}`);
    if (cond) {
        return prev;
    }
    return 0;
}

function main(): int {
    let ok = 1;

    // `+` 字符串拼接：标量自动转字符串
    ok = check(ok, "n=" + 42 == "n=42", "concat_int");
    ok = check(ok, "f=" + 1.5 == "f=1.5", "concat_float");
    ok = check(ok, "b=" + true == "b=true", "concat_bool");
    ok = check(ok, "c=" + 'x' == "c=x", "concat_char");
    ok = check(ok, 42 + "!" == "42!", "concat_left_int");
    ok = check(ok, "a" + "b" + 1 == "ab1", "concat_chain");

    // std/array：int
    const nums = [5, 1, 4, 2, 3];
    sort_int(nums);
    ok = check(ok, nums[0] == 1, "sort_int_first");
    ok = check(ok, nums[4] == 5, "sort_int_last");
    ok = check(ok, min_int(nums) == 1, "min_int");
    ok = check(ok, max_int(nums) == 5, "max_int");
    ok = check(ok, sum_int(nums) == 15, "sum_int");
    reverse_int(nums);
    ok = check(ok, nums[0] == 5, "reverse_int");

    // std/array：float
    const floats = [3.5, 1.5, 2.5];
    sort_float(floats);
    ok = check(ok, floats[0] == 1.5, "sort_float");
    ok = check(ok, min_float(floats) == 1.5, "min_float");
    ok = check(ok, max_float(floats) == 3.5, "max_float");
    ok = check(ok, sum_float(floats) == 7.5, "sum_float");
    reverse_float(floats);
    ok = check(ok, floats[0] == 3.5, "reverse_float");

    // std/array：string
    const words = ["pear", "apple", "banana"];
    sort_string(words);
    ok = check(ok, words[0] == "apple", "sort_string");
    reverse_string(words);
    ok = check(ok, words[0] == "pear", "reverse_string");
    const dups = ["a", "b", "a", "c", "b"];
    const uniq = unique_string(dups);
    ok = check(ok, uniq.length == 3, "unique_len");
    ok = check(ok, uniq[0] == "a", "unique_0");
    ok = check(ok, uniq[1] == "b", "unique_1");
    ok = check(ok, uniq[2] == "c", "unique_2");

    // std/map
    const m = map_new();
    ok = check(ok, map_len(m) == 0, "map_len_0");
    ok = check(ok, map_set(m, "name", "sw") == 0, "map_set");
    ok = check(ok, map_set(m, "count", "42") == 0, "map_set2");
    ok = check(ok, map_len(m) == 2, "map_len");
    ok = check(ok, map_has(m, "name"), "map_has");
    ok = check(ok, !map_has(m, "missing"), "map_has_false");
    ok = check(ok, (map_get(m, "name") ?? "") == "sw", "map_get");
    ok = check(ok, (map_get(m, "missing") ?? "") == "", "map_get_missing");
    ok = check(ok, map_set(m, "name", "sw2") == 0, "map_set_overwrite");
    ok = check(ok, (map_get(m, "name") ?? "") == "sw2", "map_get_overwrite");
    const keys = map_keys(m);
    ok = check(ok, keys.length == 2, "map_keys_len");
    ok = check(ok, keys[0] == "name", "map_keys_0");
    ok = check(ok, keys[1] == "count", "map_keys_1");
    ok = check(ok, map_remove(m, "count") == 0, "map_remove");
    ok = check(ok, map_remove(m, "count") == -1, "map_remove_missing");
    ok = check(ok, map_len(m) == 1, "map_len_after_remove");

    println(`final=${ok == 1 ? "PASS" : "FAIL"}`);
    return ok == 1 ? 0 : 1;
}
