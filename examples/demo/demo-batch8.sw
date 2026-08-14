import { println, print_format } from "std/io";
import { crc32, sha1, hmac_sha256, crc16 } from "std/hash";
import { time_ago_cn, month_name, weekday_name, days_between } from "std/time";
import { net_listen, net_port, net_accept, net_connect_timeout, net_send_all, net_recv, net_recv_until_close, net_close, net_peer_ip, net_resolve } from "std/net";
import { set_new, set_add, set_to_array, set_union } from "std/set";
import { csv_read_all, csv_write_all } from "std/csv";
import { chunk_int } from "std/array";
import { format } from "std/string";

// 标准库批 8 演示：net 超时/keepalive、hash 校验向量、time 人性化、集合/CSV/分块。
function main(): int {
    println("== hash 校验 ==");
    print_format("crc32(hello)=%08x  crc16(123456789)=%04x", crc32("hello"), crc16("123456789"));
    println("");
    println(sha1("abc"));
    println(hmac_sha256("key", "The quick brown fox jumps over the lazy dog"));

    println("== time 人性化 ==");
    println(time_ago_cn(90));
    println(time_ago_cn(7200));
    println(format("%s %s", month_name(8), weekday_name(3)));
    println(format("days_between=%d", days_between(0, 259200)));

    println("== net 回环（超时连接 + 读到关闭） ==");
    const server = net_listen(0);
    const port = net_port(server);
    const client = net_connect_timeout("127.0.0.1", port, 2000);
    const peer = net_accept(server);
    net_send_all(client, "ping-from-timeout-connect");
    const echo = net_recv(peer, 1024);
    const ip = net_peer_ip(peer);
    net_send_all(peer, "pong");
    net_close(peer);
    println(format("echo=%s peer_ip=%s pong=%s", echo, ip, net_recv_until_close(client)));
    net_close(client);
    net_close(server);
    println(format("resolve(localhost)=%s", net_resolve("localhost")));

    println("== 集合 / CSV / 分块 ==");
    const a = set_new();
    set_add(a, "x");
    set_add(a, "y");
    const b = set_new();
    set_add(b, "y");
    set_add(b, "z");
    println(format("union=%s", join(set_to_array(set_union(a, b)))));

    const text = "name,age\nsw,1\n\"小李,同学\",3\n";
    const rows = csv_read_all(text);
    println(format("csv rows=%d first=%s", rows.length, rows[0][0]));
    println(format("csv roundtrip=%d", csv_read_all(csv_write_all(rows)).length));

    const chunks = chunk_int([1, 2, 3, 4, 5, 6, 7], 3);
    let last = "";
    for (const item of chunks[2]) {
        if (last != "") {
            last = last + ",";
        }
        last = last + format("%d", item);
    }
    println(format("chunks=%d last=%s", chunks.length, last));
    return 0;
}

function join(items: string[]): string {
    let result = "";
    for (const item of items) {
        if (result != "") {
            result = result + ",";
        }
        result = result + item;
    }
    return result;
}
