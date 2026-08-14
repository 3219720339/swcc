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

/// 四舍五入到最近整数（.5 远离零）。
export extern c function round(value: float): float;

/// 向零截断取整。
export extern c function trunc(value: float): float;

/// 符号：负数 -1、零 0、正数 1。
export extern c function sign(value: float): float;

/// 正弦（弧度）。
export extern c function sin(value: float): float;

/// 余弦（弧度）。
export extern c function cos(value: float): float;

/// 正切（弧度）。
export extern c function tan(value: float): float;

/// 反正弦（弧度，值域 [-π/2, π/2]）。
export extern c function asin(value: float): float;

/// 反余弦（弧度，值域 [0, π]）。
export extern c function acos(value: float): float;

/// 反正切（弧度，值域 [-π/2, π/2]）。
export extern c function atan(value: float): float;

/// 四象限反正切 atan2(y, x)，返回 [-π, π]。
export extern c function atan2(y: float, x: float): float;

/// e 的 value 次方。
export extern c function exp(value: float): float;

/// 自然对数。
export extern c function log(value: float): float;

/// 以 2 为底的对数。
export extern c function log2(value: float): float;

/// 以 10 为底的对数。
export extern c function log10(value: float): float;

/// 直角三角形斜边长度 sqrt(a²+b²)。
export extern c function hypot(a: float, b: float): float;

/// 立方根。
export extern c function cbrt(value: float): float;

/// 两数较小者（float）。
export extern c function fmin(a: float, b: float): float;

/// 两数较大者（float）。
export extern c function fmax(a: float, b: float): float;

/// [0, 1) 均匀随机浮点。
export extern c function rand_float(): float;

/// [min, max) 均匀随机浮点；min >= max 时返回 min。
export extern c function rand_range(min: float, max: float): float;

/// 圆周率 π。
export extern c function pi(): float;

/// 自然常数 e。
export extern c function e(): float;
