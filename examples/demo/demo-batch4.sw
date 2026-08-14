import { println } from "std/io";
import {
    utc_year_of,
    utc_month_of,
    utc_day_of,
    utc_hour_of,
    add_months,
    add_years,
    time_from_parts,
    year_of,
    month_of,
    day_of,
    datetime_string,
    取UTC年份,
    月份增减,
} from "std/time";
import { os_which, mkdtemp, 查找可执行文件 } from "std/os";
import { is_dir } from "std/fs";
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

function main(): int {
    // UTC 字段
    const ts = time_from_parts(2024, 1, 24, 9, 12, 1);
    println(utc_year_of(ts));
    println(utc_month_of(ts));
    println(utc_day_of(ts));
    println(utc_hour_of(ts));
    println(取UTC年份(ts));

    // 日历加减
    const jan31 = time_from_parts(2024, 1, 31, 10, 0, 0);
    println(datetime_string(add_months(jan31, 1)));
    println(datetime_string(月份增减(jan31, -1)));
    println(year_of(add_years(jan31, 2)));
    println(day_of(add_months(jan31, 1)));

    // which / mkdtemp
    println(os_which("cmd"));
    println(查找可执行文件("cmd").length > 0);
    const dir = mkdtemp("sw-tmp-");
    println(dir);
    println(is_dir(dir));

    // UDP
    const s1 = udp_socket();
    const s2 = udp_socket();
    udp_bind(s2, 0);
    const port = net_port(s2);
    println(port);
    udp_send(s1, "127.0.0.1", port, "udp-hello");
    println(udp_recv(s2, 1024));
    udp_close(s1);
    udp_close(s2);
    const s3 = 创建UDPSocket();
    udp_bind(s3, 0);
    const p3 = net_port(s3);
    发送UDP数据(s3, "127.0.0.1", p3, "x");
    println(接收UDP数据(s3, 64));
    udp_close(s3);

    // regex 捕获
    println(regex_replace("2026-08-15", "(\\d+)-(\\d+)-(\\d+)", "$3/$2/$1"));
    println(正则替换("abc123", "([a-z]+)(\\d+)", "$1-$2"));
    return 0;
}
