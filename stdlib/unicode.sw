// Sw 标准库：UTF-8 按字符（Unicode 码点）工具（由运行时实现）。

export extern c function utf8_len(text: string): int;
export extern c function utf8_char_at(text: string, index: int): int;
export extern c function utf8_substring(text: string, start: int, count: int): string;
