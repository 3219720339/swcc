// ===========================================================================
// std/hash —— 字符串哈希
//
// 用法：
//   import { fnv1a_64, fnv1a_64_seed, djb2 } from "std/hash";
//   const h1 = fnv1a_64("hello");         // 64 位 FNV-1a
//   const h2 = fnv1a_64_seed("hello", 0); // 带自定义种子
//   const h3 = djb2("hello");             // DJB2
//
// 说明：
//   - 返回值为 int（64 位有符号；FNV-1a 的 64 位偏移基数为通常内建值）。
//   - 用作 map 键/去重/指纹时注意只比较同类哈希值。
// ===========================================================================

/// FNV-1a 64 位哈希（标准 64 位素数+偏移基数）。
export extern c function fnv1a_64(text: string): int;

/// FNV-1a 64 位哈希，带自定义初始种子。
export extern c function fnv1a_64_seed(text: string, seed: int): int;

/// DJB2 字符串哈希（初始 5381）。
export extern c function djb2(text: string): int;
