// ===========================================================================
// std/math —— 数学函数
//
// 用法：
//   import { abs, sqrt, floor, min, max } from "std/math";
//   import { mean_float, median_int, variance_float, stdev_float } from "std/math";
//   const x = abs(-5);          // 5
//   const y = sqrt(16.0);       // 4.0
//   const z = floor(2.7);       // 2.0
//
// 注意：取整/开方/绝对值等浮点函数均返回 float（f64）。
// 数组统计：mean/median/variance/stdev 返回 float；空数组按
// 0 / 0.0 处理（不抛异常）；min/max/sum 转发 std/array 的实现。
// ===========================================================================

import {
    sort_int,
    sort_float,
    sum_int as array_sum_int,
    sum_float as array_sum_float,
    min_int as array_min_int,
    max_int as array_max_int,
    min_float as array_min_float,
    max_float as array_max_float,
} from "std/array";

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

/// 阶乘：factorial(5) == 120；n<0 或 n>20 返回 0（溢出保护）。
export function factorial(n: int): int {
    if (n < 0 || n > 20) {
        return 0;
    }
    let result = 1;
    let i = 2;
    while (i <= n) {
        result = result * i;
        i++;
    }
    return result;
}

/// 是否为素数（n<2 为 false）。
export function is_prime(n: int): bool {
    if (n < 2) {
        return false;
    }
    if (n == 2 || n == 3) {
        return true;
    }
    if (n % 2 == 0 || n % 3 == 0) {
        return false;
    }
    let i = 5;
    while (i * i <= n) {
        if (n % i == 0 || n % (i + 2) == 0) {
            return false;
        }
        i += 6;
    }
    return true;
}

/// 大于 n 的下一个素数（n<2 返回 2）。
export function next_prime(n: int): int {
    let candidate = n < 2 ? 2 : n + 1;
    while (!is_prime(candidate)) {
        candidate++;
    }
    return candidate;
}

/// 百分比：percent(25, 200) == 12.5；total==0 返回 0。
export function percent(value: int, total: int): float {
    if (total == 0) {
        return 0.0;
    }
    return value as float * 100.0 / total as float;
}

/// 数值级四舍五入：round_to(3.14159, 2) == 3.14；digits 限制 0-6。
export function round_to(value: float, digits: int): float {
    const d = digits < 0 ? 0 : (digits > 6 ? 6 : digits);
    let factor = 1.0;
    let i = 0;
    while (i < d) {
        factor = factor * 10.0;
        i++;
    }
    return round(value * factor) / factor;
}

/// 线性插值：lerp(0.0, 10.0, 0.5) == 5.0。
export function lerp(a: float, b: float, t: float): float {
    return a + (b - a) * t;
}

export function 取阶乘(n: int): int {
    return factorial(n);
}

export function 是否素数(n: int): bool {
    return is_prime(n);
}

export function 下一个素数(n: int): int {
    return next_prime(n);
}

export function 取百分比(value: int, total: int): float {
    return percent(value, total);
}

export function 按位数舍入(value: float, digits: int): float {
    return round_to(value, digits);
}

export function 线性插值(a: float, b: float, t: float): float {
    return lerp(a, b, t);
}

// ---------------------------------------------------------------------------
// 数组统计（纯 Sw）：均值 / 中位数 / 方差 / 标准差；min/max/sum 转发 std/array。
// 约定：所有统计函数对空数组返回 0.0；方差与标准差为总体（除以 n）。
// ---------------------------------------------------------------------------

/// int[] 平均值；空数组返回 0.0。示例：mean_int([1,2,3,4]) == 2.5。
export function mean_int(items: int[]): float {
    const n = items.length;
    if (n == 0) {
        return 0.0;
    }
    let total = 0.0;
    let i = 0;
    while (i < n) {
        total = total + items[i] as float;
        i++;
    }
    return total / n as float;
}

/// float[] 平均值；空数组返回 0.0。示例：mean_float([1.0,2.0,3.0]) == 2.0。
export function mean_float(items: float[]): float {
    const n = items.length;
    if (n == 0) {
        return 0.0;
    }
    let total = 0.0;
    let i = 0;
    while (i < n) {
        total = total + items[i];
        i++;
    }
    return total / n as float;
}

/// int[] 中位数（排序后取中间；偶数个取中间两数平均）；空数组返回 0.0。
export function median_int(items: int[]): float {
    const n = items.length;
    if (n == 0) {
        return 0.0;
    }
    const sorted = items[0 : n];
    sort_int(sorted);
    if (n % 2 == 1) {
        return sorted[n / 2] as float;
    }
    const hi = n / 2;
    return (sorted[hi - 1] as float + sorted[hi] as float) / 2.0;
}

/// float[] 中位数（排序后取中间；偶数个取中间两数平均）；空数组返回 0.0。
export function median_float(items: float[]): float {
    const n = items.length;
    if (n == 0) {
        return 0.0;
    }
    const sorted = items[0 : n];
    sort_float(sorted);
    if (n % 2 == 1) {
        return sorted[n / 2];
    }
    const hi = n / 2;
    return (sorted[hi - 1] + sorted[hi]) / 2.0;
}

/// int[] 总体方差（除以 n）；空数组返回 0.0。示例：variance_int([2,4,4,4,5,5,7,9]) == 4.0。
export function variance_int(items: int[]): float {
    const n = items.length;
    if (n == 0) {
        return 0.0;
    }
    const m = mean_int(items);
    let sum_sq = 0.0;
    let i = 0;
    while (i < n) {
        const diff = items[i] as float - m;
        sum_sq = sum_sq + diff * diff;
        i++;
    }
    return sum_sq / n as float;
}

/// float[] 总体方差（除以 n）；空数组返回 0.0。
export function variance_float(items: float[]): float {
    const n = items.length;
    if (n == 0) {
        return 0.0;
    }
    const m = mean_float(items);
    let sum_sq = 0.0;
    let i = 0;
    while (i < n) {
        const diff = items[i] - m;
        sum_sq = sum_sq + diff * diff;
        i++;
    }
    return sum_sq / n as float;
}

/// int[] 总体标准差（方差的平方根）；空数组返回 0.0。
export function stdev_int(items: int[]): float {
    return sqrt(variance_int(items));
}

/// float[] 总体标准差（方差的平方根）；空数组返回 0.0。
export function stdev_float(items: float[]): float {
    return sqrt(variance_float(items));
}

/// int[] 元素之和；空数组返回 0（转发 std/array 实现）。
export function sum_int(items: int[]): int {
    return array_sum_int(items);
}

/// float[] 元素之和；空数组返回 0.0（转发 std/array 实现）。
export function sum_float(items: float[]): float {
    return array_sum_float(items);
}

/// int[] 最小值；空数组返回 0（转发 std/array 实现）。
export function min_int(items: int[]): int {
    return array_min_int(items);
}

/// int[] 最大值；空数组返回 0（转发 std/array 实现）。
export function max_int(items: int[]): int {
    return array_max_int(items);
}

/// float[] 最小值；空数组返回 0.0（转发 std/array 实现）。
export function min_float(items: float[]): float {
    return array_min_float(items);
}

/// float[] 最大值；空数组返回 0.0（转发 std/array 实现）。
export function max_float(items: float[]): float {
    return array_max_float(items);
}

// ---------------------------------------------------------------------------
// 中文函数名（数组统计，火山风格命名）
// ---------------------------------------------------------------------------

export function 取平均值整数(items: int[]): float {
    return mean_int(items);
}

export function 取平均值小数(items: float[]): float {
    return mean_float(items);
}

export function 取中位数整数(items: int[]): float {
    return median_int(items);
}

export function 取中位数小数(items: float[]): float {
    return median_float(items);
}

export function 取方差整数(items: int[]): float {
    return variance_int(items);
}

export function 取方差小数(items: float[]): float {
    return variance_float(items);
}

export function 取标准差整数(items: int[]): float {
    return stdev_int(items);
}

export function 取标准差小数(items: float[]): float {
    return stdev_float(items);
}

export function 取数组和整数(items: int[]): int {
    return sum_int(items);
}

export function 取数组和小数(items: float[]): float {
    return sum_float(items);
}

export function 取数组最小值整数(items: int[]): int {
    return min_int(items);
}

export function 取数组最大值整数(items: int[]): int {
    return max_int(items);
}

export function 取数组最小值小数(items: float[]): float {
    return min_float(items);
}

export function 取数组最大值小数(items: float[]): float {
    return max_float(items);
}
