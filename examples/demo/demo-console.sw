import { println, input, input_int, print_format } from "std/io";
import { format } from "std/string";
import {
    console_color,
    console_reset,
    console_clear,
    console_width,
    console_height,
} from "std/console";

// 交互控制台演示：input/input_int/print_format + ANSI 颜色 + 终端尺寸。
// 运行（示例输入）：
//   swc run examples/demo/demo-console.sw   （交互输入姓名与年龄）
function main(): int {
    console_clear();
    console_color(2, -1);
    println("== Sw 交互控制台演示 ==");
    console_reset();
    println(format("终端尺寸：%d x %d", console_width(), console_height()));
    const name = input("你的名字？");
    const age = input_int("你的年龄？");
    println("");
    console_color(4, -1);
    print_format("你好，%s！明年你就 %d 岁了。", name, age + 1);
    console_reset();
    println("");
    println("演示结束。");
    return 0;
}
