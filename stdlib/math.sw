// ===========================================================================
// std/math —— 数学函数
//
// 用法：
//   import { abs, sqrt, floor, min, max } from "std/math";
//   const x = abs(-5);          // 5
//   const y = sqrt(16.0);       // 4.0
//   const z = floor(2.7);       // 2.0
//
// 注意：取整/开方/绝对值等浮点函数均返回 float（f64）。
// ===========================================================================

/// 整数绝对值：abs(-5) == 5。
export extern c function abs(value: int): int;

/// 浮点绝对值：fabs(-2.5) == 2.5。
export extern c function fabs(value: float): float;

/// 向下取整：floor(2.7) == 2.0。
export extern c function floor(value: float): float;

/// 向上取整：ceil(2.1) == 3.0。
export extern c function ceil(value: float): float;

/// 平方根：sqrt(16.0) == 4.0；负数返回 NaN。
export extern c function sqrt(value: float): float;

/// 取两个整数中较小者。
export extern c function min(a: int, b: int): int;

/// 取两个整数中较大者。
export extern c function max(a: int, b: int): int;

/// 伪随机数，范围 [0, max)；max <= 0 返回 0。
export extern c function rand_int(max: int): int;

/// 把 value 限制在 [lo, hi] 区间。
export extern c function clamp(value: int, lo: int, hi: int): int;

/// 最大公约数。
export extern c function gcd(a: int, b: int): int;

/// 最小公倍数；任一为 0 返回 0。
export extern c function lcm(a: int, b: int): int;
