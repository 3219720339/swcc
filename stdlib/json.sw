// Sw 标准库：最小 JSON 解析（由运行时实现）。
// 值类型 kind：0 null / 1 bool / 2 int / 3 float / 4 string / 5 array / 6 object。

export extern c function json_parse(text: string): ptr<void>;
export extern c function json_kind(value: ptr<void>): int;
export extern c function json_bool(value: ptr<void>): int;
export extern c function json_int(value: ptr<void>): int;
export extern c function json_float(value: ptr<void>): float;
export extern c function json_string(value: ptr<void>): string;
export extern c function json_array_len(value: ptr<void>): int;
export extern c function json_array_at(value: ptr<void>, index: int): ptr<void>;
export extern c function json_object_get(value: ptr<void>, key: string): ptr<void>;
