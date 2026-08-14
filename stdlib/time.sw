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

/// 阻塞当前线程指定毫秒数；非正数立即返回。
export extern c function sleep_ms(milliseconds: int): void;
