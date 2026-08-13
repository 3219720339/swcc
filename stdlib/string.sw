// Sw 标准库：字符串方法（由运行时实现）。

export extern c function index_of(text: string, needle: string): int;
export extern c function contains(text: string, needle: string): bool;
export extern c function starts_with(text: string, prefix: string): bool;
export extern c function substring(text: string, start: int, length: int): string;
