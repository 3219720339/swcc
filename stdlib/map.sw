// ===========================================================================
// std/map —— 字符串键字典（句柄风格，键值均为 string）
//
// 用法：
//   import { map_new, map_set, map_get, map_has, map_remove, map_len, map_keys } from "std/map";
//   const m = map_new();
//   map_set(m, "name", "sw");
//   const v = map_get(m, "name");   // string?，键不存在返回 null
//   map_has(m, "name");             // true
//   map_len(m);                     // 1
//   const keys = map_keys(m);       // ["name"]（插入顺序）
//
// 说明：
//   - map 由 GC 管理，无需手动释放；值类型为 string（v0.1）。
//   - map_set 对已存在键覆盖更新；map_keys 返回键数组（插入顺序）。
// ===========================================================================

/// 创建空 map，返回句柄（ptr<void>）。
export extern c function map_new(): ptr<void>;

/// 设置键值（已存在则覆盖）；成功返回 0，失败返回 -1。
export extern c function map_set(map: ptr<void>, key: string, value: string): int;

/// 读取键对应值；键不存在返回 null。
export extern c function map_get(map: ptr<void>, key: string): string?;

/// 键是否存在。
export extern c function map_has(map: ptr<void>, key: string): bool;

/// 删除键；成功返回 0，键不存在返回 -1。
export extern c function map_remove(map: ptr<void>, key: string): int;

/// 键数量。
export extern c function map_len(map: ptr<void>): int;

/// 全部键（string[]，插入顺序）。
export extern c function map_keys(map: ptr<void>): string[];
