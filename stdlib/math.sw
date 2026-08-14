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

/// 角度转弧度。
export extern c function deg_to_rad(degrees: float): float;

/// 弧度转角度。
export extern c function rad_to_deg(radians: float): float;

/// 是否为 NaN。
export extern c function is_nan(value: float): bool;

/// 是否为正负无穷。
export extern c function is_infinite(value: float): bool;

/// 2π。
export extern c function tau(): float;

/// [min, max) 范围内的随机整数；max <= min 返回 min。
export extern c function rand_int_range(min: int, max: int): int;

/// 随机布尔值（true/false）。
export extern c function rand_bool(): bool;

/// UUID v4 文本（36 字符，如 "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx"）。
export extern c function random_uuid(): string;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 取随机整数范围(min: int, max: int): int {
    return rand_int_range(min, max);
}

export function 取随机布尔(): bool {
    return rand_bool();
}

export function 取随机UUID(): string {
    return random_uuid();
}

/// 随机字母数字字符串（A-Z a-z 0-9，length 个字符）。
export extern c function random_string(length: int): string;

/// 随机十六进制 token（2*length 个 hex 字符，用于验证码/令牌）。
export extern c function random_token(length: int): string;

export function 取随机字符串(length: int): string {
    return random_string(length);
}

export function 取随机令牌(length: int): string {
    return random_token(length);
}
