// Sw 标准库：字符串方法（由运行时实现）。

export extern c function index_of(text: string, needle: string): int;
export extern c function contains(text: string, needle: string): bool;
export extern c function starts_with(text: string, prefix: string): bool;
export extern c function substring(text: string, start: int, length: int): string;
export extern c function to_upper(text: string): string;
export extern c function to_lower(text: string): string;
export extern c function trim(text: string): string;
export extern c function split(text: string, separator: string): string[];
export extern c function join(items: string[], separator: string): string;
export extern c function replace(text: string, from: string, to: string): string;
export extern c function parse_int(text: string): int;
export extern c function parse_float(text: string): float;
