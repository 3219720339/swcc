import { println } from "std/io";
import {
    map_new,
    map_set,
    map_get,
    map_has,
    map_remove,
    map_len,
    map_clear,
    map_keys,
    map_values,
    map_set_int,
    map_get_int,
    map_inc,
    map_set_float,
    map_get_float,
    map_set_bool,
    map_get_bool,
    取整数,
    计数累加,
} from "std/map";
import { join } from "std/string";

function main(): int {
    const m = map_new();
    map_set(m, "name", "sw");
    map_set(m, "year", "2026");
    map_set(m, "lang", "cn");
    println(`len=${map_len(m)} has_name=${map_has(m, "name")} has_missing=${map_has(m, "missing")}`);
    const got_name = map_get(m, "name") ?? "(null)";
    const got_missing = map_get(m, "missing") ?? "(null)";
    println(`get_name=${got_name} get_missing=${got_missing}`);
    println(`keys=${join(map_keys(m), ",")} values=${join(map_values(m), ",")}`);
    map_set(m, "name", "swcc");
    const after_name = map_get(m, "name") ?? "(null)";
    println(`after_set name=${after_name}`);
    map_remove(m, "year");
    println(`after_remove len=${map_len(m)} has_year=${map_has(m, "year")}`);
    map_clear(m);
    println(`after_clear len=${map_len(m)}`);

    // 任意类型值
    const typed = map_new();
    map_set_int(typed, "count", 10);
    map_inc(typed, "count", 5);
    map_inc(typed, "fresh", 3);
    map_set_float(typed, "score", 3.14);
    map_set_bool(typed, "enabled", true);
    println(map_get_int(typed, "count", 0));
    println(map_get_int(typed, "missing", -1));
    println(计数累加(typed, "count", 2));
    println(map_get_float(typed, "score", 0.0));
    println(map_get_bool(typed, "enabled", false));
    println(取整数(typed, "count", 0));
    println(map_len(typed));
    return 0;
}
