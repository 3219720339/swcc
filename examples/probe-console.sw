import { println, input, input_int, input_float, print_format } from "std/io";
import { format } from "std/string";
import {
    console_color,
    console_reset,
    console_clear,
    console_gotoxy,
    console_hide_cursor,
    console_show_cursor,
    console_width,
    console_height,
} from "std/console";

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

    // print_format：直接输出（对照 format 验证同一规格的文本）
    print_format("%d-%s-%.2f", 42, "x", 3.14);
    println("");
    passed = passed & check(
        format("%d-%s-%.2f", 42, "x", 3.14) == "42-x-3.14",
        "print_format spec matches format"
    );
    print_format("%5d|%-10s|%08x", 42, "ab", 255);
    println("");
    passed = passed & check(
        format("%5d|%-10s|%08x", 42, "ab", 255) == "   42|ab        |000000ff",
        "print_format width/precision"
    );

    // input / input_int / input_float：stdin 由 run-examples.py 管道提供
    // "hello\n42\nbad\n7\n3.5\n"：bad 触发 input_int 重试读 7
    const name = input("name? ");
    passed = passed & check(name == "hello", "input reads line");
    const n = input_int("n? ");
    passed = passed & check(n == 42, "input_int parses");
    const m = input_int("m? ");
    passed = passed & check(m == 7, "input_int retries on invalid input");
    const f = input_float("f? ");
    passed = passed & check(f == 3.5, "input_float parses");

    // console：ANSI 调用不崩溃；终端尺寸在重定向（CI 管道）时为 0，断言 >= 0
    console_color(2, -1);
    println("green foreground");
    console_color(-1, 4);
    println("blue background");
    console_color(-1, -1);
    console_reset();
    console_gotoxy(1, 1);
    console_hide_cursor();
    console_show_cursor();
    console_clear();
    passed = passed & check(console_width() >= 0, "console_width >= 0");
    passed = passed & check(console_height() >= 0, "console_height >= 0");

    // getch 不做自动化断言（交互终端才可读；管道下 POSIX 返回 -1）

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
