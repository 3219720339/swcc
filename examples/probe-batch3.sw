import { println } from "std/io";
import {
    ini_parse,
    ini_save,
    ini_get,
    ini_set,
    解析配置文件,
    保存配置文件,
    取配置项,
    置配置项,
} from "std/ini";
import { random_string, random_token, 取随机字符串, 取随机令牌 } from "std/math";
import { json_parse, json_stringify, json_stringify_pretty, JSON美化输出 } from "std/json";
import {
    arr_range,
    arr_fill,
    arr_count_int,
    arr_avg_int,
    取整数序列,
    填充数组,
    统计出现次数,
    取数组平均值,
} from "std/array";
import { map_get, map_new, map_set } from "std/map";
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

    // ---- INI 解析 ----
    const cfg = ini_parse("[server]\nport = 8080\nhost = 127.0.0.1\n# 注释\n; 分号注释\ndebug = true\n");
    passed = passed & check((map_get(cfg, "server.port") ?? "") == "8080", "ini section key");
    passed = passed & check((map_get(cfg, "server.host") ?? "") == "127.0.0.1", "ini host");
    passed = passed & check((map_get(cfg, "server.debug") ?? "") == "true", "ini in section");
    const cfg_nosection = ini_parse("debug = true\n");
    passed = passed & check((map_get(cfg_nosection, "debug") ?? "") == "true", "ini no section");
    passed = passed & check((map_get(cfg, "server.missing") ?? "none") == "none", "ini missing");
    passed = passed & check((ini_get(cfg, "server.port") ?? "") == "8080", "ini_get wrapper");
    ini_set(cfg, "server.timeout", "30");
    passed = passed & check((ini_get(cfg, "server.timeout") ?? "") == "30", "ini_set wrapper");
    const text = ini_save(cfg);
    passed = passed & check(text.index_of("port=8080") >= 0, "ini_save section key");
    passed = passed & check(text.index_of("debug=true") >= 0, "ini_save no section");
    const cfg2 = 解析配置文件("a=1\n[b]\nc=2\n");
    passed = passed & check((取配置项(cfg2, "a") ?? "") == "1", "cn parse");
    passed = passed & check((取配置项(cfg2, "b.c") ?? "") == "2", "cn get");
    置配置项(cfg2, "d", "3");
    passed = passed & check(保存配置文件(cfg2).index_of("d=3") >= 0, "cn set/save");

    // ---- 随机字符串 ----
    const rs = random_string(16);
    passed = passed & check(rs.length == 16, "random_string length");
    let all_alnum = true;
    let ri = 0;
    while (ri < rs.length) {
        const c = rs[ri];
        if (!((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9'))) {
            all_alnum = false;
        }
        ri = ri + 1;
    }
    passed = passed & check(all_alnum, "random_string alnum");
    const rt = random_token(8);
    passed = passed & check(rt.length == 16, "random_token length");
    const rt2 = 取随机令牌(4);
    passed = passed & check(rt2.length == 8, "cn token");
    const rs2 = 取随机字符串(6);
    passed = passed & check(rs2.length == 6, "cn random string");
    passed = passed & check(random_string(0) == "", "random_string zero");

    // ---- JSON 美化 ----
    const doc = json_parse(`{"a":1,"b":[1,2],"c":{"x":true}}`);
    const pretty = json_stringify_pretty(doc);
    passed = passed & check(pretty.index_of("\n") >= 0, "pretty newline");
    passed = passed & check(pretty.index_of("  ") >= 0, "pretty indent");
    passed = passed & check(JSON美化输出(doc).index_of("\n") >= 0, "cn pretty");
    const flat = json_stringify(doc);
    passed = passed & check(flat.index_of("\n") < 0, "compact no newline");

    // ---- 数组实用 ----
    const r1 = arr_range(1, 5, 1);
    passed = passed & check(r1.length == 4 && r1[0] == 1 && r1[3] == 4, "arr_range up");
    const r2 = arr_range(5, 1, -2);
    passed = passed & check(r2.length == 2 && r2[0] == 5 && r2[1] == 3, "arr_range down");
    const r3 = arr_range(0, 3, 0);
    passed = passed & check(r3.length == 0, "arr_range zero step");
    const f = arr_fill(7, 3);
    passed = passed & check(f.length == 3 && f[0] == 7 && f[2] == 7, "arr_fill");
    const cnt = arr_count_int([1, 2, 1, 3, 1], 1);
    passed = passed & check(cnt == 3, "arr_count");
    const avg = arr_avg_int([1, 2, 3]);
    passed = passed & check(avg == 2.0, "arr_avg");
    passed = passed & check(取整数序列(1, 4, 1).length == 3, "cn range");
    passed = passed & check(填充数组(0, 2).length == 2, "cn fill");
    passed = passed & check(统计出现次数([1, 1, 2], 1) == 2, "cn count");
    passed = passed & check(取数组平均值([4, 6]) == 5.0, "cn avg");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
