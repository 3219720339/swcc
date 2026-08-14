import { println } from "std/io";
import {
    net_listen,
    net_port,
    net_connect,
    net_accept,
    net_send,
    net_recv,
    net_close,
} from "std/net";

function check(prev: int, cond: bool, label: string): int {
    let state = "FAIL";
    if (cond) {
        state = "ok";
    }
    println(`[${state}] ${label}`);
    if (cond) {
        return prev;
    }
    return 0;
}

function main(): int {
    let ok = 1;
    const server = net_listen(0);
    ok = check(ok, server >= 0, "net_listen");
    const port = net_port(server);
    ok = check(ok, port > 0, "net_port");
    const client = net_connect("127.0.0.1", port);
    ok = check(ok, client >= 0, "net_connect");
    const peer = net_accept(server);
    ok = check(ok, peer >= 0, "net_accept");
    ok = check(ok, net_send(client, "hello") == 5, "net_send");
    ok = check(ok, net_recv(peer, 16) == "hello", "net_recv");
    ok = check(ok, net_send(peer, "world") == 5, "net_send2");
    ok = check(ok, net_recv(client, 16) == "world", "net_recv2");
    ok = check(ok, net_close(peer) == 0, "net_close_peer");
    ok = check(ok, net_close(client) == 0, "net_close_client");
    ok = check(ok, net_close(server) == 0, "net_close_server");
    const refused = net_connect("127.0.0.1", 1);
    ok = check(ok, refused < 0, "net_connect_refused");
    println(`final=${ok == 1 ? "PASS" : "FAIL"}`);
    return ok == 1 ? 0 : 1;
}
