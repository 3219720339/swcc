// probe-cross-stdlib.sw：主文件与 lib-std.sw 各自都使用基础标准库，
// 主文件自己处理字符串/数组/map/时间/文件等，再调用被调用模块。
import { println } from "std/io";
import {
    to_upper,
    reverse,
    format,
    join,
    split,
    format_float,
} from "std/string";
import { gcd, sqrt, pi, rand_int_range } from "std/math";
import { datetime_string, date_string, now_sec } from "std/time";
import { sort_int, sum_int, unique_string } from "std/array";
import { map_new, map_set, map_get, map_set_int, map_get_int, map_inc } from "std/map";
import { json_parse, json_string, json_int, json_object_get } from "std/json";
import { md5, sha256 } from "std/hash";
import { regex_find, regex_find_all } from "std/regex";
import { read_lines, write_lines, path_join, exists } from "std/fs";
import { base64_encode, hex_encode } from "std/encoding";
import {
    describe,
    math_report,
    today,
    now_text,
    array_sum,
    unique_words,
    counter_report,
    json_report,
    hash_report,
    regex_report,
    file_report,
    encode_report,
    random_report,
} from "./lib-std";

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

    // —— 主文件自己使用基础库 ——
    // 字符串
    const mine_upper = to_upper("sw");
    const mine_rev = reverse("sw");
    passed = passed & check(mine_upper == "SW" && mine_rev == "ws", "main string");
    const mine_split = split("a,b,c", ",");
    passed = passed & check(join(mine_split, "|") == "a|b|c", "main split/join");

    // 数学
    passed = passed & check(format("gcd=%d sqrt=%.0f", gcd(12, 18), sqrt(16.0)) == "gcd=6 sqrt=4", "main math");
    const mine_rand = rand_int_range(1, 11);
    passed = passed & check(mine_rand >= 1 && mine_rand <= 10, "main rand range");

    // 时间
    const mine_date = date_string(now_sec());
    passed = passed & check(mine_date.length == 10, "main date");
    const mine_dt = datetime_string(now_sec());
    passed = passed & check(mine_dt.length == 19, "main datetime");

    // 数组
    const mine_nums = [5, 2, 3];
    sort_int(mine_nums);
    passed = passed & check(sum_int(mine_nums) == 10 && mine_nums[0] == 2, "main array");
    passed = passed & check(join(unique_string(["x", "y", "x"]), ",") == "x,y", "main unique");

    // map（任意类型值）
    const mine_map = map_new();
    map_set(mine_map, "name", "sw");
    map_set_int(mine_map, "count", 1);
    map_inc(mine_map, "count", 9);
    passed = passed & check((map_get(mine_map, "name") ?? "") == "sw" && map_get_int(mine_map, "count", 0) == 10, "main map");

    // JSON
    const mine_doc = json_parse(`{"ok":true,"n":7}`);
    passed = passed & check(json_string(json_object_get(mine_doc, "ok")) == "true" || json_int(json_object_get(mine_doc, "n")) == 7, "main json");

    // 哈希
    passed = passed & check(md5("hello") == "5d41402abc4b2a76b9719d911017c592", "main md5");
    passed = passed & check(sha256("hello").length == 64, "main sha256");

    // 正则
    const mine_all = regex_find_all("a1 b22", "\\d+");
    passed = passed & check(mine_all.length == 2 && regex_find("a1 b22", "\\d+") == "1", "main regex");

    // 文件（主文件自己也读写一个文件）
    const mine_file = path_join(".", "main-lib.txt");
    write_lines(mine_file, ["主文件", "第二行"]);
    const mine_lines = read_lines(mine_file);
    passed = passed & check(exists(mine_file) == 1 && mine_lines.length == 2 && mine_lines[0] == "主文件", "main fs");

    // 编码
    passed = passed & check(base64_encode("hi") == "aGk=" && hex_encode("hi") == "6869", "main encoding");

    // —— 调用 lib-std.sw（被调用模块内部也全用基础库） ——
    passed = passed & check(describe("sw") == "SW-ws", "describe string stdlib");
    passed = passed & check(math_report() == "gcd=6 sqrt=4 pi=3.14", "math stdlib");
    const d = today();
    passed = passed & check(d.length == 10, "date stdlib");
    const t = now_text();
    passed = passed & check(t.length == 19, "datetime stdlib");

    const nums = [3, 1, 2];
    passed = passed & check(array_sum(nums) == 6, "array stdlib");
    passed = passed & check(unique_words(["a", "b", "a"]) == "a,b", "unique stdlib");
    passed = passed & check(counter_report() == "name=sw count=5", "map stdlib");
    passed = passed & check(json_report() == "lang=sw year=2026", "json stdlib");
    passed = passed & check(hash_report() == "md5=5d41402abc4b2a76b9719d911017c592 sha256_len=64", "hash stdlib");
    passed = passed & check(regex_report("a1 b22 c333") == "first=1 all=3", "regex stdlib");
    passed = passed & check(file_report(".") == "exists=1 lines=3 first=one", "fs stdlib");
    passed = passed & check(encode_report() == "b64=aGk= hex=6869", "encoding stdlib");
    const r = random_report();
    passed = passed & check(r.starts_with("rand="), "random stdlib");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
