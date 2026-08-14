import { println } from "std/io";
import {
    now_ms,
    now_sec,
    date_string,
    datetime_string,
    parse_date,
    time_format,
    time_from_parts,
    timezone_offset_sec,
    parse_datetime,
    year_of,
    month_of,
    day_of,
    hour_of,
    minute_of,
    second_of,
    weekday_of,
    weekday_cn,
    time_string,
    iso_string,
    format_duration,
    days_in_month,
    days_in_year,
    shift_time,
    time_diff,
    uptime_ms,
    取现行时间戳,
    取年份,
    取星期文本,
    秒数转时间格式,
    时间戳增减,
} from "std/time";

function main(): int {
    const now = now_sec();
    println(`now_sec=${now} now_ms=${now_ms()}`);
    println(`date=${date_string(now)}`);
    println(`datetime=${datetime_string(now)}`);
    println(`format=${time_format(now, "%Y-%m-%d %H:%M:%S %A")}`);
    println(`year=${year_of(now)} month=${month_of(now)} day=${day_of(now)}`);
    println(`hour=${hour_of(now)} minute=${minute_of(now)} second=${second_of(now)}`);
    println(`weekday=${weekday_of(now)} weekday_cn=${weekday_cn(now)}`);
    println(`time_string=${time_string(now)}`);
    println(`iso_string=${iso_string(now)}`);
    println(`tz_offset_sec=${timezone_offset_sec()}`);
    println(`parse_date=2026-01-02 -> ${parse_date("2026-01-02")}`);
    println(`parse_datetime=2026-01-02 03:04:05 -> ${parse_datetime("2026-01-02 03:04:05")}`);

    const ts = time_from_parts(2024, 12, 24, 9, 12, 1);
    println(`time_from_parts -> ${datetime_string(ts)}`);
    println(`duration 90 -> ${format_duration(90, 0, 0)}`);
    println(`duration 90 中文 -> ${format_duration(90, 0, 1)}`);
    println(`duration 90061 含天 -> ${format_duration(90061, 1, 0)}`);
    println(`days_in_month(2024,2)=${days_in_month(2024, 2)} days_in_year(2023)=${days_in_year(2023)}`);
    println(`shift +1 day -> ${date_string(shift_time(ts, 1, 0, 0, 0))}`);
    println(`time_diff 0秒=${time_diff(ts, ts + 60, 0)} 1分=${time_diff(ts, ts + 60, 1)} 2时=${time_diff(ts, ts + 3600, 2)} 3天=${time_diff(ts, ts + 86400 * 3, 3)}`);
    println(`uptime_ms=${uptime_ms()}`);

    // 中文函数名
    println(`取现行时间戳=${取现行时间戳()} 取年份=${取年份(ts)} 取星期文本=${取星期文本(ts)}`);
    println(`秒数转时间格式=${秒数转时间格式(90, 0, 1)} 时间戳增减=${date_string(时间戳增减(ts, 2, 0, 0, 0))}`);
    return 0;
}
