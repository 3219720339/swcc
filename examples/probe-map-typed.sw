import { println } from "std/io";
import {
    map_new,
    map_set,
    map_get,
    map_has,
    map_len,
    map_set_int,
    map_get_int,
    map_inc,
    map_set_float,
    map_get_float,
    map_set_bool,
    map_get_bool,
    置整数,
    取整数,
    计数累加,
    置小数,
    取小数,
    置逻辑,
    取逻辑,
} from "std/map";
import { format_float } from "std/string";

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

    // int 值
    map_set_int(m, "count", 10);
    passed = passed & check(map_get_int(m, "count", 0) == 10, "set/get int");
    passed = passed & check(map_get_int(m, "missing", -1) == -1, "int fallback");

    // 计数累加
    map_inc(m, "count", 5);
    passed = passed & check(map_get_int(m, "count", 0) == 15, "inc existing");
    passed = passed & check(map_inc(m, "fresh", 3) == 3, "inc init");
    passed = passed & check(map_get_int(m, "fresh", 0) == 3, "inc stored");
    passed = passed & check(计数累加(m, "词频", 1) == 1, "cn inc");
    passed = passed & check(取整数(m, "词频", 0) == 1, "cn get int");

    // float 值
    map_set_float(m, "score", 3.14);
    passed = passed & check(map_get_float(m, "score", 0.0) == 3.14, "set/get float");
    passed = passed & check(map_get_float(m, "missing", -1.0) == -1.0, "float fallback");
    passed = passed & check(取小数(m, "score", 0.0) == 3.14, "cn get float");
    passed = passed & check(format_float(置小数(m, "ratio", 0.5) == 0 ? 0.5 : 0.0, 1) == "0.5", "cn set float");

    // bool 值
    map_set_bool(m, "enabled", true);
    passed = passed & check(map_get_bool(m, "enabled", false), "set/get bool");
    passed = passed & check(!map_get_bool(m, "missing", false), "bool fallback");
    passed = passed & check(取逻辑(m, "enabled", false), "cn get bool");

    // string 值仍可用（混合）
    map_set(m, "name", "sw");
    passed = passed & check((map_get(m, "name") ?? "") == "sw", "string value intact");
    passed = passed & check(map_len(m) == 7, "mixed map length");
    passed = passed & check(map_has(m, "count") && map_has(m, "name"), "mixed keys");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
