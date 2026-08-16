// ===========================================================================
// std/json —— 最小 JSON 解析
//
// 用法：
//   import { json_parse, json_object_get, json_string, json_array_at } from "std/json";
//   const doc = json_parse(`{"name": "sw", "tags": ["a", "b"]}`);
//   const name = json_string(json_object_get(doc, "name"));
//   const first = json_string(json_array_at(json_object_get(doc, "tags"), 0));
//
// 值表示：所有 JSON 值统一为 ptr<void>，用 json_kind 区分类型：
//   0 null / 1 bool / 2 int / 3 float / 4 string / 5 array / 6 object
// 访问不存在的键/越界索引会返回 null（kind 0）。
// 注意：当前不支持转义序列里的 \uXXXX（其余 \n \t \" 等支持）。
// ===========================================================================

import { split } from "std/string";
import { read_all, atomic_write } from "std/fs";

/// 解析 JSON 文本；语法错误返回 null。结果由 GC 管理，无需手动释放。
export extern c function json_parse(text: string): ptr<void>;

/// 返回值的类型编号：0 null / 1 bool / 2 int / 3 float / 4 string / 5 array / 6 object。
export extern c function json_kind(value: ptr<void>): int;

/// bool 值（0/1）。
export extern c function json_bool(value: ptr<void>): int;

/// int 值。
export extern c function json_int(value: ptr<void>): int;

/// float 值。
export extern c function json_float(value: ptr<void>): float;

/// string 值。
export extern c function json_string(value: ptr<void>): string;

/// 数组长度。
export extern c function json_array_len(value: ptr<void>): int;

/// 数组第 index 个元素；越界返回 null。
export extern c function json_array_at(value: ptr<void>, index: int): ptr<void>;

/// 取对象中 key 对应的值；键不存在返回 null。
export extern c function json_object_get(value: ptr<void>, key: string): ptr<void>;

/// 把解析后的 JSON 值序列化为紧凑 JSON 文本。
export extern c function json_stringify(value: ptr<void>): string;

/// 对象的所有键（string[]）；非对象返回空数组。
export extern c function json_object_keys(value: ptr<void>): string[];

/// JSON 值类型名："null"/"bool"/"int"/"float"/"string"/"array"/"object"。
export extern c function json_type_name(value: ptr<void>): string;

/// 把 JSON 值序列化为缩进换行的美化文本（缩进 2 空格）。
export extern c function json_stringify_pretty(value: ptr<void>): string;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function JSON美化输出(value: ptr<void>): string {
    return json_stringify_pretty(value);
}

/// 创建空 JSON 对象（配合 json_object_set 构建）。
export extern c function json_object_new(): ptr<void>;

/// 创建空 JSON 数组（配合 json_array_append 构建）。
export extern c function json_array_new(): ptr<void>;

/// 创建 JSON 字符串节点。
export extern c function json_string_new(text: string): ptr<void>;

/// 创建 JSON 整数节点。
export extern c function json_int_new(value: int): ptr<void>;

/// 创建 JSON 浮点节点。
export extern c function json_float_new(value: float): ptr<void>;

/// 创建 JSON 布尔节点。
export extern c function json_bool_new(value: bool): ptr<void>;

/// 创建 JSON null 节点。
export extern c function json_null_new(): ptr<void>;

/// 给 JSON 对象设置键值（覆盖同名键）；成功返回 0，失败返回 -1。
export extern c function json_object_set(object: ptr<void>, key: string, value: ptr<void>): int;

/// 给 JSON 数组追加元素；成功返回 0，失败返回 -1。
export extern c function json_array_append(array: ptr<void>, value: ptr<void>): int;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 创建JSON对象(): ptr<void> {
    return json_object_new();
}

export function 创建JSON数组(): ptr<void> {
    return json_array_new();
}

export function 创建JSON文本(text: string): ptr<void> {
    return json_string_new(text);
}

export function 创建JSON整数(value: int): ptr<void> {
    return json_int_new(value);
}

export function 创建JSON小数(value: float): ptr<void> {
    return json_float_new(value);
}

export function 创建JSON逻辑(value: bool): ptr<void> {
    return json_bool_new(value);
}

export function 创建JSON空值(): ptr<void> {
    return json_null_new();
}

export function JSON对象置值(object: ptr<void>, key: string, value: ptr<void>): int {
    return json_object_set(object, key, value);
}

export function JSON数组追加(array: ptr<void>, value: ptr<void>): int {
    return json_array_append(array, value);
}

/// 按点路径访问嵌套对象：json_get_path(obj, "a.b.c")。
/// 任一层不是对象/键不存在返回 null；仅支持对象路径（不含数组下标）。
export function json_get_path(value: ptr<void>, path: string): ptr<void> {
    let current = value;
    const parts = split(path, ".");
    for (const part of parts) {
        if (current == null) {
            return null;
        }
        current = json_object_get(current, part);
    }
    return current;
}

/// 对象是否包含指定键（值为 null 也算包含）；非对象返回 false。
export function json_has(value: ptr<void>, key: string): bool {
    return json_object_get(value, key) != null;
}

/// 对象的键数量；非对象返回 0。
export function json_object_len(value: ptr<void>): int {
    return json_object_keys(value).length;
}

/// 合并两个 JSON 对象，返回新对象（b 的同名键覆盖 a）；非对象按空对象处理。
export function json_merge(a: ptr<void>, b: ptr<void>): ptr<void> {
    const result = json_object_new();
    const keys_a = json_object_keys(a);
    for (const key of keys_a) {
        json_object_set(result, key, json_object_get(a, key));
    }
    const keys_b = json_object_keys(b);
    for (const key of keys_b) {
        json_object_set(result, key, json_object_get(b, key));
    }
    return result;
}

export function 取JSON路径(value: ptr<void>, path: string): ptr<void> {
    return json_get_path(value, path);
}

export function JSON是否含键(value: ptr<void>, key: string): bool {
    return json_has(value, key);
}

export function 取JSON对象长度(value: ptr<void>): int {
    return json_object_len(value);
}

export function JSON合并(a: ptr<void>, b: ptr<void>): ptr<void> {
    return json_merge(a, b);
}

/// 读取并解析 JSON 文件；不存在或无效文件返回 null。
export function json_read_file(path: string): ptr<void> {
    return json_parse(read_all(path));
}

/// 原子写入紧凑 JSON；成功返回写入字节数，失败返回 -1。
export function json_write_file(path: string, value: ptr<void>): int {
    return atomic_write(path, json_stringify(value));
}

/// 原子写入缩进 JSON；适合人工维护的配置文件。
export function json_write_file_pretty(path: string, value: ptr<void>): int {
    return atomic_write(path, json_stringify_pretty(value));
}
