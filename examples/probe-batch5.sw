import { println } from "std/io";
import {
    json_object_new,
    json_array_new,
    json_string_new,
    json_int_new,
    json_float_new,
    json_bool_new,
    json_null_new,
    json_object_set,
    json_array_append,
    json_stringify,
    json_parse,
    json_object_get,
    json_array_at,
    json_int,
    json_string,
    json_bool,
    创建JSON对象,
    创建JSON数组,
    创建JSON文本,
    创建JSON整数,
    JSON对象置值,
    JSON数组追加,
} from "std/json";
import { render_template, 模板渲染 } from "std/string";
import { format_table, 格式化表格 } from "std/table";
import { map_new, map_set } from "std/map";

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

    // ---- JSON 构建 ----
    const obj = json_object_new();
    json_object_set(obj, "name", json_string_new("sw"));
    json_object_set(obj, "year", json_int_new(2026));
    json_object_set(obj, "pi", json_float_new(3.14));
    json_object_set(obj, "ok", json_bool_new(true));
    json_object_set(obj, "nil", json_null_new());
    const tags = json_array_new();
    json_array_append(tags, json_string_new("gc"));
    json_array_append(tags, json_string_new("vtable"));
    json_object_set(obj, "tags", tags);

    const text = json_stringify(obj);
    passed = passed & check(text.index_of("\"name\":\"sw\"") >= 0, "stringify name");
    passed = passed & check(text.index_of("\"year\":2026") >= 0, "stringify int");
    passed = passed & check(text.index_of("\"ok\":true") >= 0, "stringify bool");
    passed = passed & check(text.index_of("\"nil\":null") >= 0, "stringify null");
    passed = passed & check(text.index_of("\"tags\":[\"gc\",\"vtable\"]") >= 0, "stringify array");

    // 覆盖同名键
    json_object_set(obj, "year", json_int_new(2030));
    const text2 = json_stringify(obj);
    passed = passed & check(text2.index_of("\"year\":2030") >= 0, "object set overwrite");

    // 回读验证
    const back = json_parse(text2);
    passed = passed & check(json_string(json_object_get(back, "name")) == "sw", "parse back name");
    passed = passed & check(json_int(json_object_get(back, "year")) == 2030, "parse back year");
    passed = passed & check(json_bool(json_object_get(back, "ok")) == 1, "parse back bool");
    const back_tags = json_object_get(back, "tags");
    passed = passed & check(json_string(json_array_at(back_tags, 1)) == "vtable", "parse back tags");

    // 中文名
    const cobj = 创建JSON对象();
    JSON对象置值(cobj, "k", 创建JSON文本("v"));
    const ctext = json_stringify(cobj);
    passed = passed & check(ctext == "{\"k\":\"v\"}", "cn object");
    const carr = 创建JSON数组();
    JSON数组追加(carr, 创建JSON整数(7));
    passed = passed & check(json_stringify(carr) == "[7]", "cn array");

    // ---- 模板渲染 ----
    const m = map_new();
    map_set(m, "name", "sw");
    map_set(m, "n", "42");
    passed = passed & check(render_template("Hi {name}, n={n}", m) == "Hi sw, n=42", "template basic");
    passed = passed & check(render_template("{{name}}", m) == "{name}", "template escape");
    passed = passed & check(render_template("x={missing}", m) == "x=", "template unknown empty");
    passed = passed & check(模板渲染("{name}-{n}", m) == "sw-42", "cn template");

    // ---- 表格格式化 ----
    const headers = ["名字", "数量"];
    const rows = [
        ["a", "1"],
        ["bb", "22"],
        ["ccc", "333"],
    ];
    const table = format_table(headers, rows);
    passed = passed & check(table.index_of("名字 数量") >= 0, "table header");
    passed = passed & check(table.index_of("------ ------") >= 0, "table separator");
    passed = passed & check(table.index_of("a      1") >= 0, "table row1");
    passed = passed & check(table.index_of("bb     22") >= 0, "table row2");
    passed = passed & check(table.index_of("ccc    333") >= 0, "table row3");
    const ctable = 格式化表格(headers, rows);
    passed = passed & check(ctable == table, "cn table same");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
