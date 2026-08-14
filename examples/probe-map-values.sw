import { println } from "std/io";
import { map_new, map_set, map_get, map_has, map_len, map_keys, map_values } from "std/map";

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
    const m = map_new();
    map_set(m, "a", "apple");
    map_set(m, "b", "banana");
    map_set(m, "c", "cherry");

    const keys = map_keys(m);
    const vals = map_values(m);
    passed = passed & check(keys.length == 3 && keys[0] == "a" && keys[2] == "c", "map_keys order");
    passed = passed & check(vals.length == 3, "map_values length");
    passed = passed & check(vals[0] == "apple" && vals[1] == "banana" && vals[2] == "cherry", "map_values order matches keys");
    passed = passed & check((map_get(m, "b") ?? "") == "banana", "map_get still works");

    map_set(m, "a", "avocado"); // 覆盖更新
    const vals2 = map_values(m);
    passed = passed & check(vals2.length == 3 && vals2[0] == "avocado", "map_values after overwrite");

    const empty = map_new();
    const empty_vals = map_values(empty);
    passed = passed & check(empty_vals.length == 0, "map_values on empty map");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
