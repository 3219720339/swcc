// ===========================================================================
// std/time —— 时间与延时
//
// 用法：
//   import { now_ms, sleep_ms } from "std/time";
//   const start = now_ms();
//   sleep_ms(100);              // 阻塞 100ms
//   const elapsed = now_ms() - start;
// ===========================================================================

/// 返回自 Unix 纪元（1970-01-01 UTC）以来的毫秒数。
export extern c function now_ms(): int;

/// 返回自 Unix 纪元以来的秒数。
export extern c function now_sec(): int;

/// 把 Unix 秒时间戳格式化为本地日期 "YYYY-MM-DD"。
export extern c function date_string(seconds: int): string;

/// 把 Unix 秒时间戳格式化为本地日期时间 "YYYY-MM-DD HH:MM:SS"。
export extern c function datetime_string(seconds: int): string;

/// 解析 "YYYY-MM-DD" 为当天 00:00 本地时间的 Unix 秒；格式错误返回 -1。
export extern c function parse_date(text: string): int;

/// 阻塞当前线程指定毫秒数；非正数立即返回。
export extern c function sleep_ms(milliseconds: int): void;

/// 按自定义格式格式化本地时间。支持：%Y %y %m %d %H %M %S %a %A %b %B %e %p %%。
/// 示例：time_format(now_sec(), "%Y-%m-%d %H:%M:%S")
export extern c function time_format(seconds: int, fmt: string): string;

/// 由年月日时分秒构造本地时间戳（month 1-12，hour 0-23）。
export extern c function time_from_parts(
    year: int,
    month: int,
    day: int,
    hour: int,
    minute: int,
    second: int
): int;

/// 当前本地时区相对 UTC 的偏移秒数（东为正、西为负）。
export extern c function timezone_offset_sec(): int;

/// 解析 "YYYY-MM-DD[ T]HH:MM:SS" 为本地时间戳；格式错误返回 -1。
export extern c function parse_datetime(text: string): int;
