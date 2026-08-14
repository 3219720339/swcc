import { println } from "std/io";
import {
    utc_year_of,
    utc_month_of,
    utc_day_of,
    utc_hour_of,
    utc_minute_of,
    utc_second_of,
    utc_weekday_of,
    add_months,
    add_years,
    time_from_parts,
    timezone_offset_sec,
    year_of,
    month_of,
    day_of,
    datetime_string,
    取UTC年份,
    取UTC月份,
    月份增减,
    年份增减,
} from "std/time";
import { os_which, mkdtemp, 查找可执行文件, 创建临时目录 } from "std/os";
import { is_dir, list_dir } from "std/fs";
import {
    udp_socket,
    udp_bind,
    udp_send,
    udp_recv,
    udp_close,
    net_port,
    创建UDPSocket,
    发送UDP数据,
    接收UDP数据,
} from "std/net";
import { regex_replace, 正则替换 } from "std/regex";

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

    // ---- UTC 时间字段 ----
    // 用 UTC 2024-01-24 09:12:01 的时间戳（本地时区 +8 时为 17:12）
    const utc_ts = time_from_parts(2024, 1, 24, 9, 12, 1);
    passed = passed & check(year_of(utc_ts) == 2024, "local year");
    const uy = utc_year_of(utc_ts);
    const um = utc_month_of(utc_ts);
    const ud = utc_day_of(utc_ts);
    passed = passed & check(uy == 2024 && um == 1 && ud == 24, "utc date fields");
    // utc_hour = (local_hour - 时区偏移秒/3600) mod 24，跨时区恒成立。
    const tz_hours = timezone_offset_sec() / 3600;
    const expected_utc_hour = (9 - tz_hours + 24) % 24;
    passed = passed & check(utc_hour_of(utc_ts) == expected_utc_hour, "utc hour shift");
    passed = passed & check(utc_minute_of(utc_ts) == 12 && utc_second_of(utc_ts) == 1, "utc min/sec");
    const uw = utc_weekday_of(utc_ts);
    passed = passed & check(uw >= 0 && uw <= 6, "utc weekday range");
    passed = passed & check(取UTC年份(utc_ts) == 2024, "cn utc year");
    passed = passed & check(取UTC月份(utc_ts) == 1, "cn utc month");

    // ---- 日历加减 ----
    const jan31 = time_from_parts(2024, 1, 31, 10, 0, 0);
    const feb_end = add_months(jan31, 1);
    passed = passed & check(month_of(feb_end) == 2, "add_months month");
    passed = passed & check(day_of(feb_end) == 29, "add_months clamp leap");
    const mar31 = add_months(jan31, 2);
    passed = passed & check(month_of(mar31) == 3 && day_of(mar31) == 31, "add_months normal");
    const prev = add_months(jan31, -1);
    passed = passed & check(month_of(prev) == 12 && year_of(prev) == 2023, "add_months negative");
    const plus2y = add_years(jan31, 2);
    passed = passed & check(year_of(plus2y) == 2026 && month_of(plus2y) == 1, "add_years");
    const feb29 = time_from_parts(2024, 2, 29, 0, 0, 0);
    const next_year = add_years(feb29, 1);
    passed = passed & check(year_of(next_year) == 2025 && day_of(next_year) == 28, "add_years clamp");
    passed = passed & check(月份增减(jan31, 1) == feb_end, "cn add_months");
    passed = passed & check(年份增减(jan31, 2) == plus2y, "cn add_years");

    // ---- which ----
    // 跨平台：Windows 用 cmd，Linux/macOS 用 sh。
    const which_exe = os_which("cmd");
    const which_sh = os_which("sh");
    passed = passed & check(which_exe.length > 0 || which_sh.length > 0, "which found");
    passed = passed & check(查找可执行文件("cmd").length > 0 || 查找可执行文件("sh").length > 0, "cn which");
    const which_none = os_which("sw-nonexistent-xyz");
    passed = passed & check(which_none == "", "which missing empty");

    // ---- mkdtemp ----
    const dir = mkdtemp("sw-tmp-");
    passed = passed & check(dir.length > 0, "mkdtemp path");
    passed = passed & check(is_dir(dir) == 1, "mkdtemp dir exists");
    const d2 = 创建临时目录("pre-");
    passed = passed & check(is_dir(d2) == 1, "cn mkdtemp");

    // ---- UDP 回环 ----
    const s1 = udp_socket();
    const s2 = udp_socket();
    passed = passed & check(s1 >= 0 && s2 >= 0, "udp socket");
    udp_bind(s2, 0);
    const port = net_port(s2);
    passed = passed & check(port > 0, "udp bind port");
    const sent = udp_send(s1, "127.0.0.1", port, "udp-hello");
    passed = passed & check(sent == 9, "udp send");
    const recv = udp_recv(s2, 1024);
    passed = passed & check(recv == "udp-hello", "udp recv");
    udp_close(s1);
    udp_close(s2);
    const s3 = 创建UDPSocket();
    udp_bind(s3, 0);
    const p3 = net_port(s3);
    passed = passed & check(发送UDP数据(s3, "127.0.0.1", p3, "x") == 1, "cn udp send");
    passed = passed & check(接收UDP数据(s3, 64) == "x", "cn udp recv");
    udp_close(s3);

    // ---- regex 捕获引用 ----
    passed = passed & check(regex_replace("2026-08-15", "(\\d+)-(\\d+)-(\\d+)", "$3/$2/$1") == "15/08/2026", "regex $1..$3");
    passed = passed & check(regex_replace("hello", "(h)(e)", "$2$1") == "ehllo", "regex group reorder");
    passed = passed & check(正则替换("abc123", "([a-z]+)(\\d+)", "$1-$2") == "abc-123", "cn regex groups");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
