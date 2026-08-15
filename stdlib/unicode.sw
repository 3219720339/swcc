// ===========================================================================
// std/unicode —— UTF-8 按字符（Unicode 码点）工具
//
// 用法：
//   import { utf8_len, utf8_char_at, utf8_substring } from "std/unicode";
//   const s = "你好Sw";
//   utf8_len(s);          // 4
//   utf8_char_at(s, 0);   // 20320（'你' 的码点）
//   utf8_substring(s, 0, 2);  // "你好"
//
// 说明：string.length 与 s[i] 已按字符语义，这几个函数用于显式控制；
// utf8_char_at 越界返回 -1。
// ===========================================================================

/// 返回字符串的字符数（Unicode 码点个数）。
export extern c function utf8_len(text: string): int;

/// 返回第 index 个字符的码点；越界返回 -1。
export extern c function utf8_char_at(text: string, index: int): int;

/// 截取从第 start 个字符开始的 count 个字符。
export extern c function utf8_substring(text: string, start: int, count: int): string;

/// 字符串的 UTF-8 字节数（stdin 底层长度；string.length 是字符数）。
export extern c function utf8_byte_len(text: string): int;

/// 第 char_index 个字符的字节起始偏移；越界返回 -1（指向末尾返回 len）。
export extern c function utf8_index_to_byte(text: string, char_index: int): int;

/// byte_offset 所在字符的字符序号；offset 落在多字节字符中间或越界返回 -1。
export extern c function utf8_byte_to_index(text: string, byte_offset: int): int;

/// 是否全部为可打印字符（可打印 ASCII + 非 ASCII UTF-8，排除私有区段；\n\t\r 可打印）。
export extern c function utf8_is_printable(text: string): bool;

/// 是否全部为 CJK 字符（中日韩统一表意文字/扩展 A-E、兼容表意、
/// 假名、谚文；空串或含非 CJK 返回 false）。
/// 示例：is_cjk("你好") == true；is_cjk("你好a") == false。
export extern c function is_cjk(text: string): bool;

/// 是否全部为字母（Unicode 字母类：拉丁/希腊/西里尔及其扩展，
/// 以及 CJK 表意文字——Unicode 归类为字母 Lo，中文按字母处理；
/// 空串或含非字母返回 false）。数字、标点不算字母。
/// 示例：is_letter("abc") == true；is_letter("你好") == true；
///       is_letter("a1") == false。
export extern c function is_letter(text: string): bool;

/// 是否全部为数字（ASCII 0-9 + 全角数字 + 阿拉伯-印度数字等；
/// 空串或含非数字返回 false）。
/// 示例：is_digit("123") == true；is_digit("１２３") == true。
export extern c function is_digit(text: string): bool;

/// 字符串总显示宽度：CJK/全角字符记 2，其余记 1（终端对齐用）。
/// 示例：char_width("你好") == 4；char_width("ab") == 2。
export extern c function char_width(text: string): int;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 是否中文(text: string): bool {
    return is_cjk(text);
}

export function 是否字母(text: string): bool {
    return is_letter(text);
}

export function 是否数字(text: string): bool {
    return is_digit(text);
}

export function 取显示宽度(text: string): int {
    return char_width(text);
}
