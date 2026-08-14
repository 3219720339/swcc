import { println } from "std/io";
import {
    time_from_parts,
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
    取年份,
    取星期文本,
    时间戳增减,
    秒数转时间格式,
} from "std/time";

function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

function main(): int {
    let passed = 1;
    const ts = time_from_parts(2024, 1, 24, 9, 12, 1);
    passed = passed & check(year_of(ts) == 2024, "year_of");
    passed = passed & check(month_of(ts) == 1, "month_of");
    passed = passed & check(day_of(ts) == 24, "day_of");
    passed = passed & check(hour_of(ts) == 9, "hour_of");
    passed = passed & check(minute_of(ts) == 12, "minute_of");
    passed = passed & check(second_of(ts) == 1, "second_of");
    passed = passed & check(weekday_of(ts) == 3, "weekday_of wednesday");
    passed = passed & check(weekday_cn(ts) == "三", "weekday_cn");
    passed = passed & check(time_string(ts) == "09:12:01", "time_string");
    passed = passed & check(iso_string(ts).length == 19, "iso_string");

    passed = passed & check(format_duration(90, 0, 0) == "00:01:30", "duration basic");
    passed = passed & check(format_duration(90061, 1, 0) == "1天 01:01:01", "duration days");
    passed = passed & check(format_duration(90, 0, 1) == "00时01分30秒", "duration text");
    passed = passed & check(days_in_month(2024, 2) == 29, "days_in_month leap");
    passed = passed & check(days_in_month(2023, 2) == 28, "days_in_month normal");
    passed = passed & check(days_in_year(2024) == 366 && days_in_year(2023) == 365, "days_in_year");

    const tomorrow = shift_time(ts, 1, 0, 0, 0);
    passed = passed & check(day_of(tomorrow) == 25, "shift_time +1 day");
    passed = passed & check(time_diff(ts, ts + 3600, 2) == 1, "time_diff hours");
    passed = passed & check(uptime_ms() > 0, "uptime_ms");

    // 中文名
    passed = passed & check(取年份(ts) == 2024, "cn year");
    passed = passed & check(取星期文本(ts) == "三", "cn weekday");
    passed = passed & check(day_of(时间戳增减(ts, 1, 0, 0, 0)) == 25, "cn shift");
    passed = passed & check(秒数转时间格式(90, 0, 1) == "00时01分30秒", "cn duration");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
