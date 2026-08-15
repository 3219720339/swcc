// ===========================================================================
// std/map —— 字符串键字典（句柄风格，键值均为 string）
//
// 用法：
//   import { map_new, map_set, map_get, map_has, map_remove, map_len, map_keys, map_values, map_clear } from "std/map";
//   const m = map_new();
//   map_set(m, "name", "sw");
//   const v = map_get(m, "name");   // string?，键不存在返回 null
//   map_has(m, "name");             // true
//   map_len(m);                     // 1
//   const keys = map_keys(m);       // ["name"]（插入顺序）
//   const vals = map_values(m);     // ["sw"]（插入顺序）
//   map_clear(m);                   // 清空所有键值
//
// 说明：
//   - map 由 GC 管理，无需手动释放；值类型为 string（v0.1）。
//   - map_set 对已存在键覆盖更新；map_keys / map_values 返回键/值数组（插入顺序）。
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

/// 清空所有键值；成功返回 0，map 无效返回 -1。
export extern c function map_clear(map: ptr<void>): int;

/// 全部键（string[]，插入顺序）。
export extern c function map_keys(map: ptr<void>): string[];

/// 全部值（string[]，与 map_keys 同序——插入顺序）。
export extern c function map_values(map: ptr<void>): string[];

/// 设置整数键值（覆盖 string 值）；成功返回 0，失败返回 -1。
export extern c function map_set_int(map: ptr<void>, key: string, value: int): int;

/// 读取整数键值；键不存在或类型不符返回 fallback。
/// 示例：map_get_int(m, "count", 0)。
export extern c function map_get_int(map: ptr<void>, key: string, fallback: int): int;

/// 计数累加：键存在且为 int 则加 delta，否则以 delta 初始化；返回新值。
/// 示例：map_inc(m, "访问数", 1)（词频统计）。
export extern c function map_inc(map: ptr<void>, key: string, delta: int): int;

/// 设置浮点键值；成功返回 0，失败返回 -1。
export extern c function map_set_float(map: ptr<void>, key: string, value: float): int;

/// 读取浮点键值；键不存在或类型不符返回 fallback。
export extern c function map_get_float(map: ptr<void>, key: string, fallback: float): float;

/// 设置布尔键值；成功返回 0，失败返回 -1。
export extern c function map_set_bool(map: ptr<void>, key: string, value: bool): int;

/// 读取布尔键值；键不存在或类型不符返回 fallback。
export extern c function map_get_bool(map: ptr<void>, key: string, fallback: bool): bool;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 置整数(map: ptr<void>, key: string, value: int): int {
    return map_set_int(map, key, value);
}

export function 取整数(map: ptr<void>, key: string, fallback: int): int {
    return map_get_int(map, key, fallback);
}

export function 计数累加(map: ptr<void>, key: string, delta: int): int {
    return map_inc(map, key, delta);
}

export function 置小数(map: ptr<void>, key: string, value: float): int {
    return map_set_float(map, key, value);
}

export function 取小数(map: ptr<void>, key: string, fallback: float): float {
    return map_get_float(map, key, fallback);
}

export function 置逻辑(map: ptr<void>, key: string, value: bool): int {
    return map_set_bool(map, key, value);
}

export function 取逻辑(map: ptr<void>, key: string, fallback: bool): bool {
    return map_get_bool(map, key, fallback);
}

/// 读取键对应值；键不存在返回 fallback（便捷默认值）。
export function map_get_or(map: ptr<void>, key: string, fallback: string): string {
    return map_get(map, key) ?? fallback;
}

export function 取键值或默认(map: ptr<void>, key: string, fallback: string): string {
    return map_get_or(map, key, fallback);
}

/// 合并两个 map，返回新 map（b 的同名键覆盖 a）。配置合并常用。
export function map_merge(a: ptr<void>, b: ptr<void>): ptr<void> {
    const result = map_new();
    for (const key of map_keys(a)) {
        map_set(result, key, map_get(a, key) ?? "");
    }
    for (const key of map_keys(b)) {
        map_set(result, key, map_get(b, key) ?? "");
    }
    return result;
}

/// 用 keys/values 数组建 map（长度不同以短者为准）。
export function map_from_arrays(keys: string[], values: string[]): ptr<void> {
    const result = map_new();
    const n = keys.length < values.length ? keys.length : values.length;
    let i = 0;
    while (i < n) {
        map_set(result, keys[i], values[i]);
        i++;
    }
    return result;
}

export function 合并映射(a: ptr<void>, b: ptr<void>): ptr<void> {
    return map_merge(a, b);
}

export function 数组建映射(keys: string[], values: string[]): ptr<void> {
    return map_from_arrays(keys, values);
}
