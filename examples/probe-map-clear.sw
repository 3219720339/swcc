import { println } from "std/io";
import { map_new, map_set, map_get, map_has, map_len, map_keys, map_clear } from "std/map";

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
    map_set(m, "a", "1");
    map_set(m, "b", "2");
    passed = passed & check(map_len(m) == 2 && map_has(m, "a"), "map populated");

    // 清空后再 set，验证复用。
    map_clear(m);
    passed = passed & check(map_len(m) == 0, "map_clear empties");
    passed = passed & check(!map_has(m, "a"), "map_clear removes keys");

    map_set(m, "c", "3");
    passed = passed & check(map_len(m) == 1 && (map_get(m, "c") ?? "") == "3", "map reusable after clear");

    const keys = map_keys(m);
    passed = passed & check(keys.length == 1 && keys[0] == "c", "map_keys after re-set");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
