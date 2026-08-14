// ===========================================================================
// std/io —— 控制台输入输出
//
// 用法：
//   import { println, print, read_line } from "std/io";
//   println("hello");          // 输出并换行
//   println(42);               // 输出任意类型：int/float/bool/char/string
//   println(3.14, true);       // 多参数之间用空格分隔
//   print("no newline");       // 输出不换行
//   const line = read_line();  // 从 stdin 读一行（去掉行尾换行符）
//   const n = input_int("n? ");        // 提示 + 读取整数（无效自动重试）
//   print_format("%d-%s", 42, "x");    // printf 风格直接输出
//
// 注意：read_line 读取长度上限 4095 字节；读到 EOF 返回空字符串。
// input/input_int/input_float 在 EOF（空行）时分别返回 "" / 0 / 0。
// ===========================================================================

import { is_number, parse_int, parse_float } from "std/string";

/// 输出一行（自动追加换行）。参数可为任意类型（int/float/bool/char/string
/// 及表达式结果），多个参数之间用空格分隔；无参数时只输出换行。
export extern c function println(...args: any): void;

/// 输出文本，不追加换行。参数可为任意类型，多个参数之间用空格分隔。
export extern c function print(...args: any): void;

/// 从标准输入读取一行（UTF-8 文本），去掉行尾 \n 与 \r。
/// 返回 string；EOF 或失败时返回空字符串。
export extern c function read_line(): string;

/// 向标准错误输出一行并换行。
export extern c function eprintln(text: string): void;

/// 向标准错误输出（不换行）。
export extern c function eprint(text: string): void;

/// 读取标准输入全部内容直到 EOF；无输入返回空字符串。
export extern c function read_all_stdin(): string;

/// 暂停并等待按键后继续（提示"请按任意键继续..."）。
/// 用于双击运行时让控制台窗口停留，便于查看输出。
export extern c function pause(): void;

/// 输出提示（不换行）并读取一行输入，返回去除行尾换行的文本。
export function input(prompt: string): string {
    print(prompt);
    return read_line();
}

/// 输出提示并读取整数：输入无效时提示并重试；读到 EOF（空行）返回 0。
export function input_int(prompt: string): int {
    while (true) {
        print(prompt);
        const line = read_line();
        if (line == "") {
            return 0;
        }
        if (is_number(line)) {
            return parse_int(line);
        }
        println("输入无效，请重新输入");
    }
}

/// 输出提示并读取小数：输入无效时提示并重试；读到 EOF（空行）返回 0。
export function input_float(prompt: string): float {
    while (true) {
        print(prompt);
        const line = read_line();
        if (line == "") {
            return 0.0;
        }
        if (is_number(line)) {
            return parse_float(line);
        }
        println("输入无效，请重新输入");
    }
}

/// 按 printf 风格格式化后直接输出（不换行），如 print_format("%d-%s", 42, "x")。
/// 格式符与宽度/精度同 format（%d/%i/%u/%x/%X/%o/%f/%e/%g/%s/%c/%%）。
export extern c function print_format(fmt: string, ...args: any): void;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 暂停(): void {
    pause();
}

export function 输入文本(prompt: string): string {
    return input(prompt);
}

export function 输入整数(prompt: string): int {
    return input_int(prompt);
}

export function 输入小数(prompt: string): float {
    return input_float(prompt);
}

// 格式化输出：varargs 无法转发（展开只支持数组字面量），extern c 直连映射。
export extern c function 格式化输出(fmt: string, ...args: any): void;
