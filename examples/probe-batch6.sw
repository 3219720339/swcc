import { println } from "std/io";
import {
    last_index_of_string,
    last_index_of_int,
    last_index_of_float,
    min_index_int,
    max_index_int,
    zip_strings,
    取最后出现位置文本,
    取最后出现位置整数,
    取最小值位置,
    取最大值位置,
    数组配对,
} from "std/array";
import { toml_parse, toml_get, toml_set, 解析TOML, 取TOML项, 置TOML项 } from "std/toml";
import { slugify, 转网址别名 } from "std/string";
import { map_get } from "std/map";

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

    // ---- 数组：最后出现位置 ----
    passed = passed & check(last_index_of_string(["a", "b", "a", "c"], "a") == 2, "last_index string");
    passed = passed & check(last_index_of_string(["a", "b"], "x") == -1, "last_index missing");
    passed = passed & check(last_index_of_int([1, 2, 1, 3], 1) == 2, "last_index int");
    passed = passed & check(last_index_of_float([1.5, 2.5, 1.5], 1.5) == 2, "last_index float");
    passed = passed & check(取最后出现位置文本(["x", "y", "x"], "x") == 2, "cn last string");
    passed = passed & check(取最后出现位置整数([5, 6, 5], 5) == 2, "cn last int");

    // ---- 数组：极值位置 ----
    passed = passed & check(min_index_int([5, 2, 8, 1]) == 3, "min index");
    passed = passed & check(max_index_int([5, 2, 8, 1]) == 2, "max index");
    passed = passed & check(min_index_int([]) == -1, "min index empty");
    passed = passed & check(max_index_int([]) == -1, "max index empty");
    passed = passed & check(取最小值位置([3, 1, 2]) == 1, "cn min index");
    passed = passed & check(取最大值位置([3, 1, 2]) == 0, "cn max index");

    // ---- 数组：zip ----
    const z = zip_strings(["a", "b", "c"], ["1", "2"]);
    passed = passed & check(z.length == 2, "zip length shorter");
    passed = passed & check(z[0][0] == "a" && z[0][1] == "1", "zip row0");
    passed = passed & check(z[1][0] == "b" && z[1][1] == "2", "zip row1");
    const z2 = 数组配对(["x"], ["y"]);
    passed = passed & check(z2.length == 1 && z2[0][0] == "x" && z2[0][1] == "y", "cn zip");

    // ---- TOML ----
    const cfg = toml_parse("[server]\nport = 8080\nhost = \"127.0.0.1\"\nenabled = true\n# 注释\n");
    passed = passed & check((map_get(cfg, "server.port") ?? "") == "8080", "toml int");
    passed = passed & check((map_get(cfg, "server.host") ?? "") == "127.0.0.1", "toml string quoted");
    passed = passed & check((map_get(cfg, "server.enabled") ?? "") == "true", "toml bool");
    passed = passed & check((toml_get(cfg, "server.port") ?? "") == "8080", "toml_get wrapper");
    toml_set(cfg, "server.timeout", "30");
    passed = passed & check((toml_get(cfg, "server.timeout") ?? "") == "30", "toml_set wrapper");
    const cfg2 = 解析TOML("[a]\nb = \"hello\"\n");
    passed = passed & check((取TOML项(cfg2, "a.b") ?? "") == "hello", "cn toml parse");
    置TOML项(cfg2, "c", "1");
    passed = passed & check((取TOML项(cfg2, "c") ?? "") == "1", "cn toml set");

    // ---- slugify ----
    passed = passed & check(slugify("Hello World!") == "hello-world", "slug basic");
    passed = passed & check(slugify("  Multiple   Spaces  ") == "multiple-spaces", "slug collapse");
    passed = passed & check(slugify("Hello-World") == "hello-world", "slug dash");
    passed = passed & check(slugify("Sw语言 Test") == "sw语言-test", "slug chinese");
    passed = passed & check(转网址别名("Hello World!") == "hello-world", "cn slug");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
