import { println } from "std/io";
import { ini_parse, ini_save, 解析配置文件, 取配置项 } from "std/ini";
import { random_string, random_token, 取随机字符串 } from "std/math";
import { json_parse, json_stringify_pretty } from "std/json";
import { arr_range, arr_fill, arr_count_int, arr_avg_int, 取整数序列, 取数组平均值 } from "std/array";
import { map_get } from "std/map";
import { join } from "std/string";

function main(): int {
    // INI
    const cfg = ini_parse("[server]\nport = 8080\nhost = 127.0.0.1\ndebug = true\n");
    println(map_get(cfg, "server.port") ?? "(无)");
    println(map_get(cfg, "server.host") ?? "(无)");
    println(取配置项(cfg, "server.debug") ?? "(无)");
    println(ini_save(cfg));

    // 随机
    println(random_string(12));
    println(random_token(8));
    println(取随机字符串(6));

    // JSON 美化
    const doc = json_parse(`{"name":"sw","tags":["a","b"],"meta":{"v":1}}`);
    println(json_stringify_pretty(doc));

    // 数组
    const r1 = arr_range(1, 6, 1);
    println(r1.length);
    println(取整数序列(10, 1, -3).length);
    println(arr_fill(9, 4).length);
    println(arr_count_int([1, 2, 1, 3], 1));
    println(取数组平均值([10, 20, 30]));
    println(arr_avg_int([1, 2, 3, 4]));
    return 0;
}
