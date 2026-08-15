import { println } from "std/io";
import {
    net_listen,
    net_port,
    net_accept,
    net_connect_timeout,
    net_send_all,
    net_recv,
    net_recv_until_close,
    net_available,
    net_close,
    net_set_recv_timeout,
    net_set_keepalive,
    net_peer_ip,
    net_peer_port,
    net_resolve,
    net_last_error,
} from "std/net";
import { crc32, crc32_file, crc16, sha1, sha1_file, hmac_sha256 } from "std/hash";
import {
    month_name,
    weekday_name,
    time_ago_cn,
    time_ago_en,
    days_between,
    hours_between,
    minutes_between,
} from "std/time";
import { map_new, map_set, map_get_or } from "std/map";
import { set_new, set_add, set_to_array, set_union, set_intersect, set_difference, set_len } from "std/set";
import { csv_read_all, csv_write_all } from "std/csv";
import { chunk_int, chunk_float, chunk_string } from "std/array";
import { write_all, remove } from "std/fs";
import { temp_dir, platform } from "std/os";
import { path_join } from "std/fs";

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

    // ---------- std/net 增强（回环服务器） ----------
    const server = net_listen(0);
    passed = passed & check(server >= 0, "net_listen");
    const port = net_port(server);

    const client = net_connect_timeout("127.0.0.1", port, 2000);
    if (client < 0) {
        println(`net_connect_timeout 失败详情: ${net_last_error()} (port=${port})`);
    }
    passed = passed & check(client >= 0, "net_connect_timeout ok");
    if (client >= 0) {
        const peer = net_accept(server);
        passed = passed & check(peer >= 0, "net_accept");
        passed = passed & check(net_peer_ip(peer) == "127.0.0.1", "net_peer_ip");
        passed = passed & check(net_peer_port(peer) > 0, "net_peer_port > 0");
        passed = passed & check(net_set_keepalive(client, true) == 0, "net_set_keepalive");

        passed = passed & check(net_send_all(client, "hello") == 5, "net_send_all");
        passed = passed & check(net_available(peer) == 5, "net_available");
        passed = passed & check(net_recv(peer, 1024) == "hello", "net_recv echo");

        // 接收超时：服务器不发数据，客户端 300ms 后返回空串
        passed = passed & check(net_set_recv_timeout(client, 300) == 0, "net_set_recv_timeout");
        passed = passed & check(net_recv(client, 10) == "", "net_recv timeout returns empty");

        // 读到关闭：服务器发送后关闭，客户端读到全部
        net_send_all(peer, "bye");
        net_close(peer);
        passed = passed & check(net_recv_until_close(client) == "bye", "net_recv_until_close");
        net_close(client);
    }
    net_close(server);

    // 已关闭端口：connect_timeout 立即失败
    passed = passed & check(net_connect_timeout("127.0.0.1", port, 1000) == -1, "net_connect_timeout closed port");

    // 域名解析
    const resolved = net_resolve("localhost");
    passed = passed & check(resolved != "" && resolved.contains("."), "net_resolve localhost");

    // ---------- std/hash 增强（标准测试向量） ----------
    passed = passed & check(crc32("123456789") == 0xCBF43926, "crc32 check value");
    passed = passed & check(crc32("hello") == 0x3610A686, "crc32 hello");
    passed = passed & check(crc32("") == 0, "crc32 empty");
    passed = passed & check(crc16("123456789") == 0xBB3D, "crc16 check value");
    passed = passed & check(sha1("abc") == "a9993e364706816aba3e25717850c26c9cd0d89d", "sha1 abc");
    passed = passed & check(sha1("") == "da39a3ee5e6b4b0d3255bfef95601890afd80709", "sha1 empty");
    passed = passed & check(
        hmac_sha256("key", "The quick brown fox jumps over the lazy dog") ==
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8",
        "hmac_sha256 RFC 4231"
    );

    // 文件哈希（流式计算）
    const hash_dir = temp_dir();
    const hash_file = path_join(hash_dir, platform() == "windows" ? "swcc-hash8.txt" : "swcc-hash8.txt");
    write_all(hash_file, "123456789");
    passed = passed & check(crc32_file(hash_file) == 0xCBF43926, "crc32_file");
    write_all(hash_file, "abc");
    passed = passed & check(sha1_file(hash_file) == "a9993e364706816aba3e25717850c26c9cd0d89d", "sha1_file");
    remove(hash_file);

    // ---------- std/time 人性化 ----------
    passed = passed & check(month_name(3) == "March" && month_name(0) == "" && month_name(13) == "", "month_name");
    passed = passed & check(weekday_name(0) == "Sunday" && weekday_name(6) == "Saturday" && weekday_name(7) == "", "weekday_name");
    passed = passed & check(time_ago_cn(90) == "1 分钟前", "time_ago_cn minutes");
    passed = passed & check(time_ago_cn(-5) == "刚刚" && time_ago_cn(30) == "刚刚", "time_ago_cn just now");
    passed = passed & check(time_ago_cn(3600) == "1 小时前", "time_ago_cn hours");
    passed = passed & check(time_ago_cn(86400) == "1 天前", "time_ago_cn days");
    passed = passed & check(time_ago_cn(2592000) == "1 个月前", "time_ago_cn months");
    passed = passed & check(time_ago_cn(31536000) == "1 年前", "time_ago_cn years");
    passed = passed & check(time_ago_en(90) == "1 minutes ago", "time_ago_en");
    passed = passed & check(days_between(0, 172800) == 2, "days_between");
    passed = passed & check(hours_between(0, 7200) == 2, "hours_between");
    passed = passed & check(minutes_between(0, 120) == 2, "minutes_between");

    // ---------- 附加：map/set/csv/array ----------
    const m = map_new();
    map_set(m, "name", "sw");
    passed = passed & check(map_get_or(m, "name", "x") == "sw", "map_get_or found");
    passed = passed & check(map_get_or(m, "missing", "x") == "x", "map_get_or fallback");

    const sa = set_new();
    set_add(sa, "a");
    set_add(sa, "b");
    const sb = set_new();
    set_add(sb, "b");
    set_add(sb, "c");
    passed = passed & check(set_len(set_union(sa, sb)) == 3, "set_union len");
    passed = passed & check(set_to_array(set_intersect(sa, sb))[0] == "b", "set_intersect");
    passed = passed & check(set_to_array(set_difference(sa, sb))[0] == "a", "set_difference");

    const csv_text = "a,b\n\"c,d\",e\n";
    const rows = csv_read_all(csv_text);
    passed = passed & check(rows.length == 2 && rows[0][0] == "a" && rows[1][0] == "c,d", "csv_read_all");
    const roundtrip = csv_read_all(csv_write_all(rows));
    passed = passed & check(roundtrip.length == 2 && roundtrip[1][1] == "e", "csv_write_all roundtrip");

    const chunks = chunk_int([1, 2, 3, 4, 5], 2);
    passed = passed & check(chunks.length == 3 && chunks[0].length == 2 && chunks[2][0] == 5, "chunk_int");
    const cf = chunk_float([1.5, 2.5, 3.5], 2);
    passed = passed & check(cf.length == 2 && cf[0][1] == 2.5, "chunk_float");
    const cs = chunk_string(["a", "b", "c"], 2);
    passed = passed & check(cs.length == 2 && cs[1][0] == "c", "chunk_string");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
