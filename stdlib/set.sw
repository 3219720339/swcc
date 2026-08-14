// ===========================================================================
// std/set —— 字符串集合（去重，句柄风格，基于 std/map）
//
// 用法：
//   import { set_new, set_add, set_remove, set_has, set_len, set_to_array } from "std/set";
//   const s = set_new();
//   set_add(s, "apple");
//   set_add(s, "apple");      // 重复添加自动去重
//   set_has(s, "apple");      // true
//   set_len(s);               // 1
//   const all = set_to_array(s);   // ["apple"]
//
// 说明：集合元素为 string，插入顺序保持；由 GC 管理，无需手动释放。
// ===========================================================================

import { map_new, map_set, map_remove, map_has, map_len, map_keys } from "std/map";

/// 创建空集合，返回句柄（ptr<void>）。
export function set_new(): ptr<void> {
    return map_new();
}

/// 添加元素（已存在则忽略）；成功返回 0，失败返回 -1。
export function set_add(set: ptr<void>, value: string): int {
    return map_set(set, value, "");
}

/// 元素是否存在。
export function set_has(set: ptr<void>, value: string): bool {
    return map_has(set, value);
}

/// 删除元素；成功返回 0，不存在返回 -1。
export function set_remove(set: ptr<void>, value: string): int {
    return map_remove(set, value);
}

/// 元素数量。
export function set_len(set: ptr<void>): int {
    return map_len(set);
}

/// 全部元素（string[]，插入顺序）。
export function set_to_array(set: ptr<void>): string[] {
    return map_keys(set);
}

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 创建集合(): ptr<void> {
    return set_new();
}

export function 集合添加(set: ptr<void>, value: string): int {
    return set_add(set, value);
}

export function 集合是否包含(set: ptr<void>, value: string): bool {
    return set_has(set, value);
}

export function 集合删除(set: ptr<void>, value: string): int {
    return set_remove(set, value);
}

export function 集合长度(set: ptr<void>): int {
    return set_len(set);
}

export function 集合转数组(set: ptr<void>): string[] {
    return set_to_array(set);
}

/// 并集（新集合：a 全部 + b 新增，保持插入顺序）。
export function set_union(a: ptr<void>, b: ptr<void>): ptr<void> {
    const result = set_new();
    for (const item of set_to_array(a)) {
        set_add(result, item);
    }
    for (const item of set_to_array(b)) {
        set_add(result, item);
    }
    return result;
}

/// 交集（新集合：a 中同时存在于 b 的元素）。
export function set_intersect(a: ptr<void>, b: ptr<void>): ptr<void> {
    const result = set_new();
    for (const item of set_to_array(a)) {
        if (set_has(b, item)) {
            set_add(result, item);
        }
    }
    return result;
}

/// 差集（新集合：a 中存在而 b 中没有的元素）。
export function set_difference(a: ptr<void>, b: ptr<void>): ptr<void> {
    const result = set_new();
    for (const item of set_to_array(a)) {
        if (!set_has(b, item)) {
            set_add(result, item);
        }
    }
    return result;
}

export function 集合并集(a: ptr<void>, b: ptr<void>): ptr<void> {
    return set_union(a, b);
}

export function 集合交集(a: ptr<void>, b: ptr<void>): ptr<void> {
    return set_intersect(a, b);
}

export function 集合差集(a: ptr<void>, b: ptr<void>): ptr<void> {
    return set_difference(a, b);
}
