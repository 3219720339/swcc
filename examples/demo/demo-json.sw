import { println } from "std/io";
import {
    json_parse,
    json_kind,
    json_bool,
    json_int,
    json_float,
    json_string,
    json_array_len,
    json_array_at,
    json_object_get,
    json_stringify,
    json_object_keys,
    json_type_name,
} from "std/json";
import { join } from "std/string";
import { format_float } from "std/string";

function main(): int {
    const doc = json_parse(`{"lang": "sw", "year": 2026, "pi": 3.14, "ok": true, "tags": ["gc", "vtable"]}`);
    println(`type=${json_type_name(doc)} keys=${join(json_object_keys(doc), ",")}`);
    println(`lang=${json_string(json_object_get(doc, "lang"))}`);
    println(`year=${json_int(json_object_get(doc, "year"))}`);
    println(`pi=${format_float(json_float(json_object_get(doc, "pi")), 2)}`);
    println(`ok=${json_bool(json_object_get(doc, "ok"))}`);
    const tags = json_object_get(doc, "tags");
    println(`tags_len=${json_array_len(tags)} tags_0=${json_string(json_array_at(tags, 0))} tags_1=${json_string(json_array_at(tags, 1))}`);
    println(`stringify=${json_stringify(doc)}`);
    println(`missing_kind=${json_kind(json_object_get(doc, "missing"))}`);
    const bad = json_parse("{bad json");
    println(`bad_parse_null=${bad == null}`);
    return 0;
}
