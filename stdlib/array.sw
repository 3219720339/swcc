// ===========================================================================
// std/array —— 数组工具（类型化，原地操作）
//
// 用法：
//   import { sort_int, sum_int, min_int, max_int } from "std/array";
//   const nums = [5, 1, 4, 2, 3];
//   sort_int(nums);             // 原地升序 [1,2,3,4,5]
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
