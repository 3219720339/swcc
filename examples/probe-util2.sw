import { println } from "std/io";
import {
    rand_int_range,
    rand_bool,
    random_uuid,
    取随机整数范围,
    取随机布尔,
    取随机UUID,
} from "std/math";
import {
    shuffle_string,
    shuffle_int,
    打乱数组文本,
    打乱数组整数,
} from "std/array";
import {
    format_bytes,
    format_thousands,
    int_to_hex,
    int_to_oct,
    int_to_bin,
    parse_int_radix,
    to_snake_case,
    to_camel_case,
    is_alpha,
    is_alnum,
    is_punct,
    字节数格式化,
    千分位格式化,
    整数转十六进制,
    按进制解析整数,
    转蛇形命名,
    转驼峰命名,
    是否字母,
    是否字母数字,
    是否标点,
} from "std/string";
import { csv_parse_line, csv_join, 解析CSV行, 生成CSV行 } from "std/csv";
import {
    set_new,
    set_add,
    set_has,
    set_remove,
    set_len,
    set_to_array,
    创建集合,
    集合添加,
    集合是否包含,
    集合长度,
    集合转数组,
} from "std/set";
import { datetime_string_ms, week_of_year, 取日期时间毫秒, 取年份周数 } from "std/time";
import { write_lines, temp_file_path, 按行写文件, 取临时文件路径, read_lines } from "std/fs";
import { join } from "std/string";

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

    // 随机增强
    let r = rand_int_range(5, 10);
    passed = passed & check(r >= 5 && r < 10, "rand_int_range in range");
    let saw_true = false;
    let saw_false = false;
    let bi = 0;
    while (bi < 20) {
        if (rand_bool()) {
            saw_true = true;
        } else {
            saw_false = true;
        }
        bi = bi + 1;
    }
    passed = passed & check(saw_true && saw_false, "rand_bool");
    saw_true = false;
    saw_false = false;
    bi = 0;
    while (bi < 20) {
        if (取随机布尔()) {
            saw_true = true;
        } else {
            saw_false = true;
        }
        bi = bi + 1;
    }
    passed = passed & check(saw_true && saw_false, "cn rand_bool");
    const uuid = random_uuid();
    passed = passed & check(uuid.length == 36 && uuid[14] == '4', "random_uuid v4");
    passed = passed & check(uuid[8] == '-' && uuid[13] == '-' && uuid[18] == '-' && uuid[23] == '-', "uuid dashes");
    r = 取随机整数范围(0, 3);
    passed = passed & check(r >= 0 && r < 3, "cn rand range");
    passed = passed & check(取随机UUID().length == 36, "cn uuid");

    // 洗牌
    const words = ["a", "b", "c", "d", "e"];
    shuffle_string(words);
    passed = passed & check(words.length == 5, "shuffle keeps length");
    const nums = [1, 2, 3, 4, 5];
    打乱数组整数(nums);
    passed = passed & check(nums.length == 5, "cn shuffle int");

    // 格式化
    passed = passed & check(format_bytes(1536) == "1.5 KB", "format_bytes");
    passed = passed & check(format_bytes(1024) == "1 KB", "format_bytes 1024");
    passed = passed & check(format_bytes(500) == "500 B", "format_bytes B");
    passed = passed & check(format_bytes(1048576 * 5) == "5 MB", "format_bytes MB");
    passed = passed & check(format_thousands(1234567) == "1,234,567", "thousands");
    passed = passed & check(format_thousands(999) == "999", "thousands small");
    passed = passed & check(format_thousands(-1234567) == "-1,234,567", "thousands negative");
    passed = passed & check(字节数格式化(2048) == "2 KB", "cn format_bytes");
    passed = passed & check(千分位格式化(1000000) == "1,000,000", "cn thousands");

    // 进制
    passed = passed & check(int_to_hex(255) == "ff", "hex");
    passed = passed & check(int_to_hex(0) == "0", "hex zero");
    passed = passed & check(int_to_oct(8) == "10", "oct");
    passed = passed & check(int_to_bin(5) == "101", "bin");
    passed = passed & check(parse_int_radix("ff", 16) == 255, "parse hex");
    passed = passed & check(parse_int_radix("101", 2) == 5, "parse bin");
    passed = passed & check(parse_int_radix("777", 8) == 511, "parse oct");
    passed = passed & check(parse_int_radix("z", 36) == 35, "parse base36");
    passed = passed & check(整数转十六进制(255) == "ff", "cn hex");
    passed = passed & check(按进制解析整数("ff", 16) == 255, "cn parse radix");

    // 命名转换与字符分类
    passed = passed & check(to_snake_case("helloWorld") == "hello_world", "snake");
    passed = passed & check(to_snake_case("HTTPServer") == "http_server", "snake acronym");
    passed = passed & check(to_snake_case("foo-bar baz") == "foo_bar_baz", "snake separators");
    passed = passed & check(to_camel_case("hello_world") == "helloWorld", "camel");
    passed = passed & check(to_camel_case("foo-bar baz") == "fooBarBaz", "camel separators");
    passed = passed & check(转蛇形命名("helloWorld") == "hello_world", "cn snake");
    passed = passed & check(转驼峰命名("hello_world") == "helloWorld", "cn camel");
    passed = passed & check(is_alpha("abc"), "is_alpha");
    passed = passed & check(!is_alpha("ab1"), "is_alpha reject");
    passed = passed & check(is_alnum("abc123"), "is_alnum");
    passed = passed & check(!is_alnum("a b"), "is_alnum reject");
    passed = passed & check(is_punct("!?"), "is_punct");
    passed = passed & check(!is_punct("a!"), "is_punct reject");
    passed = passed & check(是否字母("abc"), "cn alpha");
    passed = passed & check(是否字母数字("abc123"), "cn alnum");
    passed = passed & check(是否标点("!?"), "cn punct");

    // CSV
    const row = csv_parse_line("a,b,\"c,d\"");
    passed = passed & check(row.length == 3 && row[0] == "a" && row[1] == "b" && row[2] == "c,d", "csv parse quote");
    const row2 = csv_parse_line("\"say \"\"hi\"\"\",plain");
    passed = passed & check(row2.length == 2 && row2[0] == "say \"hi\"" && row2[1] == "plain", "csv escaped quote");
    const row3 = csv_parse_line("x,y");
    passed = passed & check(row3.length == 2 && row3[1] == "y", "csv simple");
    passed = passed & check(csv_join(["a", "b", "c,d"]) == "a,b,\"c,d\"", "csv join quote");
    passed = passed & check(csv_join(["x", "y"]) == "x,y", "csv join simple");
    const crow = 解析CSV行("1,2,3");
    passed = passed & check(crow.length == 3 && crow[2] == "3", "cn csv parse");
    passed = passed & check(生成CSV行(["a", "b"]) == "a,b", "cn csv join");

    // Set
    const s = set_new();
    set_add(s, "apple");
    set_add(s, "apple");
    set_add(s, "banana");
    passed = passed & check(set_len(s) == 2, "set dedup");
    passed = passed & check(set_has(s, "apple"), "set has");
    passed = passed & check(!set_has(s, "cherry"), "set missing");
    const all = set_to_array(s);
    passed = passed & check(all.length == 2, "set to array");
    set_remove(s, "apple");
    passed = passed & check(set_len(s) == 1 && !set_has(s, "apple"), "set remove");
    const cs = 创建集合();
    集合添加(cs, "x");
    集合添加(cs, "x");
    passed = passed & check(集合长度(cs) == 1 && 集合是否包含(cs, "x"), "cn set");
    passed = passed & check(集合转数组(cs).length == 1, "cn set to array");

    // 时间补充
    passed = passed & check(datetime_string_ms(0) == "1970-01-01 08:00:00.000", "datetime_ms epoch cn tz");
    const ms_text = datetime_string_ms(1786700000000);
    passed = passed & check(ms_text.length == 23 && ms_text[19] == '.', "datetime_ms format");
    const w = week_of_year(1786700000);
    passed = passed & check(w >= 1 && w <= 53, "week_of_year range");
    passed = passed & check(取日期时间毫秒(1000).length == 23, "cn datetime_ms");
    passed = passed & check(取年份周数(1786700000) >= 1, "cn week");

    // 文件补充
    const lines = ["one", "two", "three"];
    passed = passed & check(write_lines("lines-tmp.txt", lines) == 14, "write_lines bytes");
    const read_back = read_lines("lines-tmp.txt");
    passed = passed & check(read_back.length == 3 && read_back[2] == "three", "write_lines roundtrip");
    passed = passed & check(按行写文件("lines-tmp2.txt", ["a", "b"]) == 4, "cn write_lines");
    const tmp = temp_file_path("sw-");
    passed = passed & check(tmp.length > 8, "temp_file_path");
    passed = passed & check(取临时文件路径("pre-").length > 6, "cn temp path");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
