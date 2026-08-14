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
    const server = net_listen(0);
    if (check(1, server >= 0, "net_listen") == 0) {
        return 1;
    }
    const port = net_port(server);
    if (check(1, port > 0, "net_port") == 0) {
        return 1;
    }
    const client = net_connect("127.0.0.1", port);
    if (check(1, client >= 0, "net_connect") == 0) {
        return 1;
    }
    const peer = net_accept(server);
    if (check(1, peer >= 0, "net_accept") == 0) {
        return 1;
    }
    if (check(1, net_send(client, "hello") == 5, "net_send") == 0) {
        return 1;
    }
    if (check(1, net_recv(peer, 16) == "hello", "net_recv") == 0) {
        return 1;
    }
    if (check(1, net_send(peer, "world") == 5, "net_send2") == 0) {
        return 1;
    }
    if (check(1, net_recv(client, 16) == "world", "net_recv2") == 0) {
        return 1;
    }
    if (check(1, net_close(peer) == 0, "net_close_peer") == 0) {
        return 1;
    }
    if (check(1, net_close(client) == 0, "net_close_client") == 0) {
        return 1;
    }
    if (check(1, net_close(server) == 0, "net_close_server") == 0) {
        return 1;
    }
    const refused = net_connect("127.0.0.1", 1);
    if (check(1, refused < 0, "net_connect_refused") == 0) {
        return 1;
    }
    println("final=PASS");
    return 0;
}
