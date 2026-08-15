import { println, print, flush } from "std/io";
import { left, right, edit_distance, ends_with_any } from "std/string";
import { factorial, is_prime, next_prime, percent, round_to, lerp } from "std/math";
import { first_int, last_int, flatten_string, avg_float } from "std/array";
import { map_new, map_set, map_get, map_merge } from "std/map";
import { memory_usage_kb } from "std/os";
import { console_title } from "std/console";
import { format } from "std/string";

// 常用基础函数演示：文本/数值/数组/map 工具。
function main(): int {
    console_title("sw demo-batch9");

    println("== 文本 ==");
    println(format("left=%s right=%s", left("hello world", 5), right("hello world", 5)));
    println(format("距离(kitten,sitting)=%d", edit_distance("kitten", "sitting")));
    println(format("是图片? %s", ends_with_any("photo.png", [".png", ".jpg", ".gif"]) ? "是" : "否"));

    println("== 数值 ==");
    println(format("5! = %d", factorial(5)));
    println(format("97 素数? %s  下一个素数(10)=%d", is_prime(97) ? "是" : "否", next_prime(10)));
    println(format("进度=%.1f%%  舍入=%.2f  插值=%.1f", percent(37, 200), round_to(3.14159, 2), lerp(0.0, 10.0, 0.3)));

    println("== 数组/map ==");
    println(format("首=%d 尾=%d 平均=%.1f", first_int([1, 2, 3], 0), last_int([1, 2, 3], 0), avg_float([1.0, 2.0, 3.0])));
    println(format("展平=%s", join(flatten_string([["a", "b"], ["c"]]))));

    const base = map_new();
    map_set(base, "host", "localhost");
    map_set(base, "port", "8080");
    const extra = map_new();
    map_set(extra, "debug", "1");
    const config = map_merge(base, extra);
    println(format("config=%s:%s debug=%s", map_get(config, "host") ?? "?", map_get(config, "port") ?? "?", map_get(config, "debug") ?? "0"));

    println("== 进程 ==");
    print(format("内存=%d KB", memory_usage_kb()));
    flush();
    println("");
    return 0;
}

function join(items: string[]): string {
    let result = "";
    for (const item of items) {
        if (result != "") {
            result = result + ",";
        }
        result = result + item;
    }
    return result;
}
