import { println } from "std/io";
import { rand_int_range, rand_bool, random_uuid, 取随机整数范围, 取随机UUID } from "std/math";
import { shuffle_string, 打乱数组文本 } from "std/array";
import {
    format_bytes,
    format_thousands,
    int_to_hex,
    int_to_bin,
    parse_int_radix,
    to_snake_case,
    to_camel_case,
    is_alpha,
    is_alnum,
    is_punct,
    字节数格式化,
    千分位格式化,
    按进制解析整数,
    转蛇形命名,
} from "std/string";
import { csv_parse_line, csv_join, 解析CSV行, 生成CSV行 } from "std/csv";
import { set_new, set_add, set_has, set_to_array, 创建集合, 集合添加, 集合转数组 } from "std/set";
import { datetime_string_ms, week_of_year, 取日期时间毫秒 } from "std/time";
import { write_lines, temp_file_path, read_lines, 取临时文件路径 } from "std/fs";
import { join } from "std/string";

function main(): int {
    // 随机
    println(rand_int_range(1, 7));
    println(rand_bool());
    println(random_uuid());
    println(取随机整数范围(10, 20));
    println(取随机UUID());

    // 洗牌
    const words = ["a", "b", "c", "d", "e"];
    shuffle_string(words);
    println(join(words, ","));
    const more = ["x", "y", "z"];
    打乱数组文本(more);
    println(join(more, ","));

    // 格式化
    println(format_bytes(1536));
    println(format_bytes(1048576 * 5));
    println(format_bytes(500));
    println(format_thousands(1234567));
    println(字节数格式化(2048));
    println(千分位格式化(99999999));

    // 进制
    println(int_to_hex(255));
    println(int_to_bin(5));
    println(parse_int_radix("ff", 16));
    println(按进制解析整数("101", 2));

    // 命名与分类
    println(to_snake_case("helloWorld"));
    println(to_camel_case("hello_world"));
    println(转蛇形命名("HTTPServer"));
    println(is_alpha("abc"));
    println(is_alnum("abc123"));
    println(is_punct("!?"));

    // CSV
    const row = csv_parse_line("a,b,\"c,d\"");
    println(row.length);
    println(join(row, "|"));
    println(csv_join(["a", "b", "c,d"]));
    const crow = 解析CSV行("1,2,3");
    println(join(crow, "+"));
    println(生成CSV行(["x", "y"]));

    // Set
    const s = set_new();
    set_add(s, "apple");
    set_add(s, "apple");
    set_add(s, "banana");
    println(set_has(s, "apple"));
    println(set_to_array(s).length);
    const cs = 创建集合();
    集合添加(cs, "one");
    集合添加(cs, "two");
    println(集合转数组(cs).length);

    // 时间
    println(datetime_string_ms(1786700000123));
    println(week_of_year(1786700000));
    println(取日期时间毫秒(1786700000456));

    // 文件
    write_lines("lines-demo.txt", ["第一行", "第二行", "第三行"]);
    const lines = read_lines("lines-demo.txt");
    println(lines.length);
    println(lines[0]);
    println(temp_file_path("sw-"));
    println(取临时文件路径("pre-"));
    return 0;
}
