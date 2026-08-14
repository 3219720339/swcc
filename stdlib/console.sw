// ===========================================================================
// std/console —— 终端控制（ANSI 颜色/清屏/光标、读键、终端尺寸）
//
// 用法：
//   import { console_color, console_clear, getch } from "std/console";
//   console_color(2, -1);      // 前景绿色（0黑 1红 2绿 3黄 4蓝 5品红 6青 7白）
//   println("green text");
//   console_reset();           // 恢复默认色
//   console_clear();           // 清屏并回左上角
//   const key = getch();       // 读一个键（不回车、不回显）
//
// 说明：
//   - 颜色/清屏/光标走 ANSI 转义序列；Windows 程序启动时自动启用 VT 模式
//     （Win10 1511+，重定向到管道/文件时自动跳过）。
//   - getch：Windows 用 _getch，POSIX 用 termios 原始模式单字节读；方向键等
//     扩展键只返回首字节。重定向（非终端）时可能返回 -1。
//   - console_width/console_height 在重定向（CI 管道/文件）时返回 0。
// ===========================================================================

/// 设置前景/背景色（0-7：黑红绿黄蓝品红青白；-1 表示该位不变；两个都为
/// -1 时重置为默认色）。输出 ANSI SGR 序列。
export extern c function console_color(foreground: int, background: int): void;

/// 重置终端颜色/样式为默认。
export extern c function console_reset(): void;

/// 清屏并把光标移到左上角。
export extern c function console_clear(): void;

/// 定位光标到 (x, y)，1 基坐标（左上角为 1,1）。
export extern c function console_gotoxy(x: int, y: int): void;

/// 隐藏光标（用于进度条/动画）。
export extern c function console_hide_cursor(): void;

/// 显示光标。
export extern c function console_show_cursor(): void;

/// 读取一个按键（不回车、不回显），返回键码（0-255）；失败返回 -1。
/// 交互式终端才能读取；重定向时 POSIX 返回 -1（Windows _getch 行为见说明）。
export extern c function getch(): int;

/// 终端宽度（字符列数）；非控制台（重定向/CI 管道）返回 0。
export extern c function console_width(): int;

/// 终端高度（字符行数）；非控制台（重定向/CI 管道）返回 0。
export extern c function console_height(): int;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 置颜色(foreground: int, background: int): void {
    console_color(foreground, background);
}

export function 重置颜色(): void {
    console_reset();
}

export function 清屏(): void {
    console_clear();
}

export function 定位光标(x: int, y: int): void {
    console_gotoxy(x, y);
}

export function 隐藏光标(): void {
    console_hide_cursor();
}

export function 显示光标(): void {
    console_show_cursor();
}

export function 读按键(): int {
    return getch();
}

export function 取终端宽度(): int {
    return console_width();
}

export function 取终端高度(): int {
    return console_height();
}
