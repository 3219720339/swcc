import { println } from "std/io";
import {
    json_parse,
    json_string,
    json_int,
    json_array_at,
    json_object_get,
} from "std/json";
import { utf8_len } from "std/unicode";

function main(): int {
    const doc = json_parse(`{"lang": "sw", "year": 2026, "features": ["gc", "vtable"]}`);
    const lang = json_string(json_object_get(doc, "lang"));
    const year = json_int(json_object_get(doc, "year"));
    const features = json_object_get(doc, "features");
    const first = json_string(json_array_at(features, 0));
    println(`lang=${lang} year=${year} first=${first} chars=${utf8_len("你好，Sw")}`);
    return 0;
}
