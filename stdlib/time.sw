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
