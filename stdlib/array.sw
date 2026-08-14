// ===========================================================================
// std/array —— 数组工具（类型化，原地操作）
//
// 用法：
//   import { sort_int, sort_int_desc, sum_int, min_int, max_int } from "std/array";
//   const nums = [5, 1, 4, 2, 3];
//   sort_int(nums);             // 原地升序 [1,2,3,4,5]
//   sort_int_desc(nums);        // 原地降序 [5,4,3,2,1]
//   const total = sum_int(nums); // 15
//   reverse_int(nums);          // 原地反转
//
// 说明：
//   - sort/reverse 原地修改传入数组；min/max/sum 只读。
//   - 空数组的 min/max 返回 0（int）/ 0.0（float），sum 返回 0。
//   - unique_string 返回去重后的新数组（保持首次出现顺序）。
// ===========================================================================

/// 原地升序排序 int[]。
export extern c function sort_int(items: int[]): void;

/// 原地升序排序 float[]。
export extern c function sort_float(items: float[]): void;

/// 原地升序排序 string[]（按 UTF-8 字节序）。
export extern c function sort_string(items: string[]): void;

/// 原地降序排序 int[]。
export extern c function sort_int_desc(items: int[]): void;

/// 原地降序排序 float[]。
export extern c function sort_float_desc(items: float[]): void;

/// 原地降序排序 string[]（按 UTF-8 字节序）。
export extern c function sort_string_desc(items: string[]): void;

/// 原地反转 int[]。
export extern c function reverse_int(items: int[]): void;

/// 原地反转 float[]。
export extern c function reverse_float(items: float[]): void;

/// 原地反转 string[]。
export extern c function reverse_string(items: string[]): void;

/// int[] 最小值；空数组返回 0。
export extern c function min_int(items: int[]): int;

/// int[] 最大值；空数组返回 0。
export extern c function max_int(items: int[]): int;

/// int[] 求和；空数组返回 0。
export extern c function sum_int(items: int[]): int;

/// float[] 最小值；空数组返回 0.0。
export extern c function min_float(items: float[]): float;

/// float[] 最大值；空数组返回 0.0。
export extern c function max_float(items: float[]): float;

/// float[] 求和；空数组返回 0.0。
export extern c function sum_float(items: float[]): float;

/// 去重 string[]，返回新数组（保持首次出现顺序）。
export extern c function unique_string(items: string[]): string[];

/// int[] 是否包含指定值。
export extern c function contains_int(items: int[], value: int): bool;

/// float[] 是否包含指定值。
export extern c function contains_float(items: float[], value: float): bool;

/// string[] 是否包含指定值（按内容）。
export extern c function contains_string(items: string[], value: string): bool;

/// int[] 中首次出现的位置；不存在返回 -1。
export extern c function index_of_int(items: int[], value: int): int;

/// float[] 中首次出现的位置；不存在返回 -1。
export extern c function index_of_float(items: float[], value: float): int;

/// string[] 中首次出现的位置；不存在返回 -1。
export extern c function index_of_string(items: string[], value: string): int;

/// 原地洗牌 string[]（随机打乱顺序）。
export extern c function shuffle_string(items: string[]): void;

/// 原地洗牌 int[]（随机打乱顺序）。
export extern c function shuffle_int(items: int[]): void;

/// 原地洗牌 float[]（随机打乱顺序）。
export extern c function shuffle_float(items: float[]): void;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 打乱数组文本(items: string[]): void {
    shuffle_string(items);
}

export function 打乱数组整数(items: int[]): void {
    shuffle_int(items);
}

export function 打乱数组小数(items: float[]): void {
    shuffle_float(items);
}

/// 生成整数序列 [start, end)，步长 step（可负）；step=0 返回空数组。
/// 示例：arr_range(1, 5, 1) == [1,2,3,4]；arr_range(5, 1, -2) == [5,3]。
export extern c function arr_range(start: int, end: int, step: int): int[];

/// 生成 count 个 value 的 int[]。示例：arr_fill(0, 3) == [0,0,0]。
export extern c function arr_fill(value: int, count: int): int[];

/// 统计 int[] 中等于 value 的元素个数。
export extern c function arr_count_int(items: int[], value: int): int;

/// int[] 平均值；空数组返回 0.0。
export extern c function arr_avg_int(items: int[]): float;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 取整数序列(start: int, end: int, step: int): int[] {
    return arr_range(start, end, step);
}

export function 填充数组(value: int, count: int): int[] {
    return arr_fill(value, count);
}

export function 统计出现次数(items: int[], value: int): int {
    return arr_count_int(items, value);
}

export function 取数组平均值(items: int[]): float {
    return arr_avg_int(items);
}
