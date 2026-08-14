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
