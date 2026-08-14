import { println, print } from "std/io";
import { date_string, now_sec } from "std/time";

function main(): int {
    // 任意类型直接输出，不加标签、不拼接、不用模板字符串
    println("hello");
    println(42);
    println(-7);
    println(3.14);
    println(true);
    println('S');
    println(date_string(now_sec()));
    println(now_sec());
    println(1 + 2 * 3);
    // 多参数：空格分隔
    println("a", "b", "c");
    println(1, 2.5, true);
    // print 不换行
    print("no");
    print(" newline");
    println();
    return 0;
}
