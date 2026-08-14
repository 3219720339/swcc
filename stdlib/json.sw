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
