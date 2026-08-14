import { println } from "std/io";
import {
    json_object_new,
    json_array_new,
    json_string_new,
    json_int_new,
    json_float_new,
    json_bool_new,
    json_object_set,
    json_array_append,
    json_stringify,
    json_stringify_pretty,
    创建JSON对象,
    JSON对象置值,
    创建JSON文本,
} from "std/json";
import { render_template, 模板渲染 } from "std/string";
import { format_table, 格式化表格 } from "std/table";
import { map_new, map_set } from "std/map";

function main(): int {
    // JSON 构建
    const obj = json_object_new();
    json_object_set(obj, "name", json_string_new("sw"));
    json_object_set(obj, "year", json_int_new(2026));
    json_object_set(obj, "pi", json_float_new(3.14));
    json_object_set(obj, "ok", json_bool_new(true));
    const tags = json_array_new();
    json_array_append(tags, json_string_new("gc"));
    json_array_append(tags, json_string_new("vtable"));
    json_object_set(obj, "tags", tags);
    println(json_stringify(obj));
    println(json_stringify_pretty(obj));
    const cobj = 创建JSON对象();
    JSON对象置值(cobj, "k", 创建JSON文本("v"));
    println(json_stringify(cobj));

    // 模板渲染
    const m = map_new();
    map_set(m, "name", "sw");
    map_set(m, "n", "42");
    println(render_template("Hi {name}, n={n}", m));
    println(render_template("{{name}} 原样", m));
    println(模板渲染("{name}-{n}", m));

    // 表格
    const headers = ["名字", "数量", "状态"];
    const rows = [
        ["sw", "1", "ok"],
        ["gc", "2", "ok"],
        ["error", "3", "fail"],
    ];
    println(format_table(headers, rows));
    println(格式化表格(headers, rows) == format_table(headers, rows));
    return 0;
}
