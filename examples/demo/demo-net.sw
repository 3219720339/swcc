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

function main(): int {
    const server = net_listen(0);
    const port = net_port(server);
    println(`listen_port=${port}`);
    const client = net_connect("127.0.0.1", port);
    println(`connect=${client != -1}`);
    const peer = net_accept(server);
    println(`accept=${peer != -1}`);
    const sent = net_send(client, "hello-from-sw");
    println(`send_bytes=${sent}`);
    const recv = net_recv(peer, 1024);
    println(`recv=${recv}`);
    net_close(peer);
    net_close(client);
    net_close(server);
    println("closed");
    return 0;
}
