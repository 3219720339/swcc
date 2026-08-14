// ===========================================================================
// std/time —— 时间与延时
//
// 用法：
//   import { now_ms, sleep_ms } from "std/time";
//   const start = now_ms();
//   sleep_ms(100);              // 阻塞 100ms
//   const elapsed = now_ms() - start;
// ===========================================================================

import { format } from "std/string";

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

/// 取本地时间的年。
export extern c function year_of(seconds: int): int;

/// 取本地时间的月（1-12）。
export extern c function month_of(seconds: int): int;

/// 取本地时间的日（1-31）。
export extern c function day_of(seconds: int): int;

/// 取本地时间的小时（0-23）。
export extern c function hour_of(seconds: int): int;

/// 取本地时间的分钟（0-59）。
export extern c function minute_of(seconds: int): int;

/// 取本地时间的秒（0-59）。
export extern c function second_of(seconds: int): int;

/// 星期几：0=周日 … 6=周六。
export extern c function weekday_of(seconds: int): int;

/// 中文星期文本：日/一/二/三/四/五/六。
export extern c function weekday_cn(seconds: int): string;

/// 本地时间 "HH:MM:SS"（补零）。
export extern c function time_string(seconds: int): string;

/// ISO 风格本地时间 "YYYY-MM-DDTHH:MM:SS"。
export extern c function iso_string(seconds: int): string;

/// 秒数转时长文本（90 → "00:01:30"；include_days 前缀 "N天 "；
/// text_mode 用 "00时01分30秒"）。
export extern c function format_duration(
    seconds: int,
    include_days: int,
    text_mode: int
): string;

/// 某年某月的天数（month 1-12）。
export extern c function days_in_month(year: int, month: int): int;

/// 某年的天数（闰年 366）。
export extern c function days_in_year(year: int): int;

/// 本地日历时间增减（days/hours/minutes/seconds，DST 安全）。
export extern c function shift_time(
    seconds: int,
    days: int,
    hours: int,
    minutes: int,
    secs: int
): int;

/// 两个时间戳间隔（unit：0秒 1分 2时 3天），结果 = sec2 - sec1。
export extern c function time_diff(sec1: int, sec2: int, unit: int): int;

/// 进程启动以来毫秒数（单调时钟）。
export extern c function uptime_ms(): int;

/// 毫秒时间戳格式化为本地日期时间含毫秒 "YYYY-MM-DD HH:MM:SS.mmm"。
export extern c function datetime_string_ms(milliseconds: int): string;

/// ISO 8601 周数（周一为一周开始；1 月 4 日所在周为第 1 周）。
export extern c function week_of_year(seconds: int): int;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 取现行时间戳(): int {
    return now_sec();
}

export function 取现行时间戳毫秒(): int {
    return now_ms();
}

export function 取年份(seconds: int): int {
    return year_of(seconds);
}

export function 取月份(seconds: int): int {
    return month_of(seconds);
}

export function 取日(seconds: int): int {
    return day_of(seconds);
}

export function 取小时(seconds: int): int {
    return hour_of(seconds);
}

export function 取分钟(seconds: int): int {
    return minute_of(seconds);
}

export function 取秒(seconds: int): int {
    return second_of(seconds);
}

export function 取星期几(seconds: int): int {
    return weekday_of(seconds);
}

export function 取星期文本(seconds: int): string {
    return weekday_cn(seconds);
}

export function 时间转文本(seconds: int): string {
    return datetime_string(seconds);
}

export function 取某月天数(year: int, month: int): int {
    return days_in_month(year, month);
}

export function 取某年天数(year: int): int {
    return days_in_year(year);
}

export function 时间戳增减(seconds: int, days: int, hours: int, minutes: int, secs: int): int {
    return shift_time(seconds, days, hours, minutes, secs);
}

export function 取时间间隔(sec1: int, sec2: int, unit: int): int {
    return time_diff(sec1, sec2, unit);
}

export function 秒数转时间格式(seconds: int, include_days: int, text_mode: int): string {
    return format_duration(seconds, include_days, text_mode);
}

export function 取系统启动时间(): int {
    return uptime_ms();
}

export function 取日期时间毫秒(milliseconds: int): string {
    return datetime_string_ms(milliseconds);
}

export function 取年份周数(seconds: int): int {
    return week_of_year(seconds);
}

/// UTC 年（不受本地时区影响）。
export extern c function utc_year_of(seconds: int): int;

/// UTC 月（1-12）。
export extern c function utc_month_of(seconds: int): int;

/// UTC 日（1-31）。
export extern c function utc_day_of(seconds: int): int;

/// UTC 小时（0-23）。
export extern c function utc_hour_of(seconds: int): int;

/// UTC 分钟（0-59）。
export extern c function utc_minute_of(seconds: int): int;

/// UTC 秒（0-59）。
export extern c function utc_second_of(seconds: int): int;

/// UTC 星期（0=周日 … 6=周六）。
export extern c function utc_weekday_of(seconds: int): int;

/// 本地日历时间加 months 个月（月末自动收敛，如 1-31 +1月 → 2-28）。
export extern c function add_months(seconds: int, months: int): int;

/// 本地日历时间加 years 年（月末自动收敛）。
export extern c function add_years(seconds: int, years: int): int;

export function 取UTC年份(seconds: int): int {
    return utc_year_of(seconds);
}

export function 取UTC月份(seconds: int): int {
    return utc_month_of(seconds);
}

export function 取UTC日(seconds: int): int {
    return utc_day_of(seconds);
}

export function 取UTC小时(seconds: int): int {
    return utc_hour_of(seconds);
}

export function 月份增减(seconds: int, months: int): int {
    return add_months(seconds, months);
}

export function 年份增减(seconds: int, years: int): int {
    return add_years(seconds, years);
}

// ---------------------------------------------------------------------------
// 人性化时间（纯 Sw 组合；相对时间/月份星期名/时间间隔）
// ---------------------------------------------------------------------------

/// 英文月份名（1-12）；越界返回空串。month_name(3) == "March"。
export function month_name(month: int): string {
    const names = [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ];
    if (month < 1 || month > 12) {
        return "";
    }
    return names[month - 1];
}

/// 英文星期名（0-6，0=星期天）；越界返回空串。weekday_name(0) == "Sunday"。
export function weekday_name(day: int): string {
    const names = [
        "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday",
    ];
    if (day < 0 || day > 6) {
        return "";
    }
    return names[day];
}

/// 中文相对时间：time_ago_cn(90) == "1 分钟前"；<=0 返回 "刚刚"。
export function time_ago_cn(seconds: int): string {
    const s = seconds < 0 ? 0 : seconds;
    if (s < 60) {
        return "刚刚";
    }
    if (s < 3600) {
        return format("%d 分钟前", s / 60);
    }
    if (s < 86400) {
        return format("%d 小时前", s / 3600);
    }
    if (s < 604800) {
        return format("%d 天前", s / 86400);
    }
    if (s < 2592000) {
        return format("%d 周前", s / 604800);
    }
    if (s < 31536000) {
        return format("%d 个月前", s / 2592000);
    }
    return format("%d 年前", s / 31536000);
}

/// 英文相对时间：time_ago_en(90) == "1 minutes ago"；<=0 返回 "just now"。
export function time_ago_en(seconds: int): string {
    const s = seconds < 0 ? 0 : seconds;
    if (s < 60) {
        return "just now";
    }
    if (s < 3600) {
        return format("%d minutes ago", s / 60);
    }
    if (s < 86400) {
        return format("%d hours ago", s / 3600);
    }
    if (s < 604800) {
        return format("%d days ago", s / 86400);
    }
    if (s < 2592000) {
        return format("%d weeks ago", s / 604800);
    }
    if (s < 31536000) {
        return format("%d months ago", s / 2592000);
    }
    return format("%d years ago", s / 31536000);
}

/// 两个 Unix 秒时间戳相差的天数（绝对值）。
export function days_between(a: int, b: int): int {
    const d = a > b ? a - b : b - a;
    return d / 86400;
}

/// 两个 Unix 秒时间戳相差的小时数（绝对值）。
export function hours_between(a: int, b: int): int {
    const d = a > b ? a - b : b - a;
    return d / 3600;
}

/// 两个 Unix 秒时间戳相差的分钟数（绝对值）。
export function minutes_between(a: int, b: int): int {
    const d = a > b ? a - b : b - a;
    return d / 60;
}

export function 取月份英文名(month: int): string {
    return month_name(month);
}

export function 取星期英文名(day: int): string {
    return weekday_name(day);
}

export function 时间差文本(seconds: int): string {
    return time_ago_cn(seconds);
}

export function 时间差文本英文(seconds: int): string {
    return time_ago_en(seconds);
}

export function 相差天数(a: int, b: int): int {
    return days_between(a, b);
}

export function 相差小时数(a: int, b: int): int {
    return hours_between(a, b);
}

export function 相差分钟数(a: int, b: int): int {
    return minutes_between(a, b);
}
