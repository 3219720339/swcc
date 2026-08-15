// probe-http-local：本地 TCP 协议探针（不依赖外部网络）。
// 双模式：spawn 自身副本（--server）作为本地 HTTP 服务器，
// 主进程作为客户端验证 http_get/http_post/http_open_timeout 等：
//   - chunked 响应解码（一次性 + keep-alive 会话）
//   - 相对/绝对 Location 重定向（最多 5 跳）
//   - 超时（服务器延迟响应，客户端 http_get_timeout 快速失败）
//   - 会话 chunked 分帧保持连接
import { println, flush } from "std/io";
import { spawn, process_poll, process_wait, process_close } from "std/os";
import { sleep_ms } from "std/time";
import { write_all, read_all, remove, append } from "std/fs";
import { net_listen, net_port, net_accept, net_recv, net_send_all, net_close } from "std/net";
import { http_get, http_post, http_get_timeout, http_post_timeout, http_status, http_body, http_open, http_open_timeout, http_request_on, http_close } from "std/http";
import { map_get_int, map_get, map_new, map_set } from "std/map";
import { url_query_map } from "std/url";
import { contains, index_of, substring, parse_int, char_at, char_code, from_code_point } from "std/string";

function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

// ---------------------------------------------------------------------------
// 服务器模式：本地 HTTP 服务器。监听随机端口，端口写入端口文件。
// 支持：Content-Length 与 chunked 响应、相对/绝对重定向、延迟响应（测超时）、
// keep-alive 多请求。
// ---------------------------------------------------------------------------
function log_line(text: string): void {
    append(".swcache/probe-http-log.txt", text + "\n");
}

function run_server(port_file: string): int {
    const server = net_listen(0);
    if (server < 0) {
        println("server: listen failed");
        return 1;
    }
    const port = net_port(server);
    write_all(port_file, `${port}`);
    log_line(`server: listening ${port}`);
    // 处理连接直到收到 /shutdown（防挂死；重定向会消耗多个连接）。
    // 上限 40 作为安全兜底，正常流程由 /shutdown 结束。
    let shutdown = false;
    for (let conn = 0; conn < 40 && !shutdown; conn++) {
        log_line(`accept #${conn}`);
        const peer = net_accept(server);
        if (peer < 0) {
            log_line(`accept #${conn} failed`);
            break;
        }
        // 处理同一连接上的多个 keep-alive 请求
        let keep = true;
        while (keep) {
            // 读请求头直到 \r\n\r\n
            let head = "";
            let guard = 0;
            while (guard < 50) {
                const chunk = net_recv(peer, 4096);
                if (chunk.length == 0) {
                    log_line(`conn#${conn} recv empty -> close`);
                    keep = false;
                    break;
                }
                head = head + chunk;
                if (contains(head, "\r\n\r\n")) {
                    break;
                }
                guard++;
            }
            if (!keep) {
                break;
            }
            const sep = index_of(head, "\r\n");
            const request_line = sep > 0 ? substring(head, 0, sep) : head;
            log_line(`conn#${conn} req=${request_line}`);
            const is_post = starts_with(request_line, "POST");
            const space = index_of(request_line, " ");
            let path = space > 0 ? substring(request_line, space + 1, request_line.length - space - 1) : "/";
            const q = index_of(path, " ");
            if (q > 0) {
                path = substring(path, 0, q);
            }
            // 读取 POST body（Content-Length 精确读取；body 可能已随 head 到达）
            if (is_post) {
                const cl = index_of(head, "Content-Length:");
                if (cl >= 0) {
                    let len_text = "";
                    let k = cl + 16;
                    while (k < head.length) {
                        const ch = char_code(head, k);
                        if (ch < 48 || ch > 57) {
                            break;
                        }
                        len_text = len_text + from_code_point(ch);
                        k++;
                    }
                    const body_len = parse_int(len_text);
                    // 计算 head 中已含的 body 字节（\r\n\r\n 之后）
                    const hdr_end = index_of(head, "\r\n\r\n");
                    let have = 0;
                    if (hdr_end >= 0) {
                        have = head.length - hdr_end - 4;
                    }
                    let need = body_len - have;
                    let guard = 0;
                    while (need > 0 && guard < 20) {
                        const more = net_recv(peer, 4096);
                        if (more.length == 0) {
                            break;
                        }
                        need = need - more.length;
                        guard++;
                    }
                }
            }
            // 路由
            if (path == "/chunked") {
                const body = "chunked-hello-world";
                const part1 = substring(body, 0, 8);
                const part2 = substring(body, 8, body.length - 8);
                const resp = "HTTP/1.1 200 OK\r\n" +
                    "Transfer-Encoding: chunked\r\n" +
                    "Connection: keep-alive\r\n" +
                    "\r\n" +
                    hex_len(part1.length) + "\r\n" + part1 + "\r\n" +
                    hex_len(part2.length) + "\r\n" + part2 + "\r\n" +
                    "0\r\n\r\n";
                net_send_all(peer, resp);
            } else if (path == "/redirect-rel") {
                net_send_all(peer,
                    "HTTP/1.1 302 Found\r\nLocation: /chunked\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n");
            } else if (path == "/redirect-abs") {
                const abs = `http://127.0.0.1:${port}/chunked`;
                net_send_all(peer,
                    "HTTP/1.1 302 Found\r\nLocation: " + abs + "\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n");
            } else if (path == "/redirect-loop") {
                net_send_all(peer,
                    "HTTP/1.1 302 Found\r\nLocation: /redirect-loop\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n");
            } else if (path == "/slow") {
                sleep_ms(800);
                net_send_all(peer,
                    "HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: keep-alive\r\n\r\nslowok");
            } else if (path == "/plain") {
                net_send_all(peer,
                    "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: keep-alive\r\n\r\nplain");
            } else if (path == "/echo-query") {
                net_send_all(peer,
                    "HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: keep-alive\r\n\r\nquery-ok-11");
            } else if (path == "/shutdown") {
                net_send_all(peer,
                    "HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nshutdown");
                keep = false;
                shutdown = true;
            } else if (path == "/close") {
                net_send_all(peer,
                    "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nplain");
                keep = false;
            } else {
                net_send_all(peer,
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n");
            }
            // 请求头里 Connection: close → 断开
            if (contains(head, "Connection: close")) {
                keep = false;
            }
        }
        net_close(peer);
    }
    net_close(server);
    remove(port_file);
    return 0;
}

function hex_len(n: int): string {
    // 十进制 → 小写十六进制
    const digits = "0123456789abcdef";
    if (n == 0) {
        return "0";
    }
    let s = "";
    let v = n;
    while (v > 0) {
        s = substring(digits, v % 16, 1) + s;
        v = v / 16;
    }
    return s;
}

function starts_with(text: string, prefix: string): bool {
    if (text.length < prefix.length) {
        return false;
    }
    for (let i = 0; i < prefix.length; i++) {
        if (char_at(text, i) != char_at(prefix, i)) {
            return false;
        }
    }
    return true;
}

// ---------------------------------------------------------------------------
// 客户端模式
// ---------------------------------------------------------------------------
function run_client(port_file: string): int {
    let passed = 1;
    println(`client: waiting for port file ${port_file}`);
    flush();
    // 等待端口文件出现（服务器启动）
    let port = 0;
    for (let i = 0; i < 100 && port == 0; i++) {
        const text = read_all(port_file);
        const trimmed = text.trim();
        if (trimmed.length > 0) {
            port = parse_int(trimmed);
        } else {
            sleep_ms(20);
        }
    }
    passed = passed & check(port > 0, "server port file");
    if (port <= 0) {
        println("server did not start");
        return 1;
    }
    const base = `http://127.0.0.1:${port}`;

    // 1) chunked 一次性响应
    const r1 = http_get(base + "/chunked");
    passed = passed & check(http_status(r1) == 200, "chunked status 200");
    passed = passed & check(http_body(r1) == "chunked-hello-world", "chunked body decode");

    // 2) 相对重定向
    const r2 = http_get(base + "/redirect-rel");
    passed = passed & check(http_status(r2) == 200, "relative redirect status 200");
    passed = passed & check(http_body(r2) == "chunked-hello-world", "relative redirect body");

    // 3) 绝对重定向
    const r3 = http_get(base + "/redirect-abs");
    passed = passed & check(http_status(r3) == 200, "absolute redirect status 200");
    passed = passed & check(http_body(r3) == "chunked-hello-world", "absolute redirect body");

    // 4) 重定向环：最多 5 跳后停在 302
    const r4 = http_get(base + "/redirect-loop");
    passed = passed & check(http_status(r4) == 302, "redirect loop bounded (302)");

    // 5) 超时：服务器延迟 800ms，客户端 200ms 超时 → status 0
    const r5 = http_get_timeout(base + "/slow", 200);
    passed = passed & check(http_status(r5) == 0, "get_timeout fails fast");

    // 6) 无超时正常拿到慢响应
    const r6 = http_get(base + "/slow");
    passed = passed & check(http_status(r6) == 200 && http_body(r6) == "slowok", "slow without timeout ok");

    // 7) keep-alive 会话：CL 分帧 + chunked 分帧 + 重定向
    const session = http_open("127.0.0.1", port);
    passed = passed & check(session >= 0, "http_open");
    const s1 = http_request_on(session, "GET", "/plain", null, "");
    passed = passed & check(http_status(s1) == 200 && http_body(s1) == "plain", "session CL body");
    const s2 = http_request_on(session, "GET", "/chunked", null, "");
    passed = passed & check(http_status(s2) == 200 && http_body(s2) == "chunked-hello-world", "session chunked body");
    const s3 = http_request_on(session, "GET", "/redirect-rel", null, "");
    passed = passed & check(http_status(s3) == 200 && http_body(s3) == "chunked-hello-world", "session relative redirect");
    http_close(session);

    // 8) 会话带超时打开
    const t0 = http_open_timeout("127.0.0.1", port, 500);
    passed = passed & check(t0 >= 0, "http_open_timeout ok");
    http_close(t0);

    // 9) POST 兼容
    const p1 = http_post(base + "/plain", "x=1");
    passed = passed & check(http_status(p1) == 200, "post status 200");
    println("client: after post");
    flush();

    // 10) URL query 便捷
    const q = url_query_map(base + "/echo-query?a=1&b=hello");
    passed = passed & check((map_get(q, "a") ?? "") == "1", "url_query_map a");
    passed = passed & check((map_get(q, "b") ?? "") == "hello", "url_query_map b");
    println("client: after url_query_map");
    flush();

    // 11) 通知服务器退出（避免 process_wait 挂起）
    println("client: before shutdown");
    flush();
    const r11 = http_get(base + "/shutdown");
    passed = passed & check(http_status(r11) == 200, "shutdown status 200");
    println("client: after shutdown");
    flush();

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}

function main(args: string[]): int {
    // args[0] 是程序名（argv[0]），spawn 时 --server 落在 args[1]
    println(`main: args.length=${args.length}`);
    for (let i = 0; i < args.length; i++) {
        println(`main: args[${i}]=${args[i]}`);
    }
    flush();
    if (args.length > 1) {
        if (args[1] == "--server") {
            const port_file = args.length > 2 ? args[2] : ".swcache/probe-http-port.txt";
            return run_server(port_file);
        }
    }
    // 客户端：spawn 自身副本为服务器
    const me = args.length > 0 ? args[0] : "";
    if (me.length == 0) {
        println("cannot locate self");
        return 1;
    }
    const port_file = ".swcache/probe-http-port.txt";
    remove(port_file);
    println(`client: spawning self [${me}]`);
    flush();
    const pid = spawn(me, ["--server", port_file]);
    println(`client: spawn pid=${pid}`);
    flush();
    if (pid <= 0) {
        println("spawn server failed");
        return 1;
    }
    const rc = run_client(port_file);
    println(`client: rc=${rc}`);
    flush();
    // 等待服务器退出并清理
    process_wait(pid);
    return rc;
}
