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
    return 0;
}
