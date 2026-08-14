// ===========================================================================
// std/net —— TCP 网络（阻塞式）
//
// 用法：
//   import { net_listen, net_port, net_connect, net_accept, net_send, net_recv, net_close } from "std/net";
//   const server = net_listen(0);          // 监听随机端口
//   const port = net_port(server);
//   const client = net_connect("127.0.0.1", port);
//   const peer = net_accept(server);
//   net_send(client, "hello");
//   const text = net_recv(peer, 1024);     // "hello"
//
// 说明：
//   - 全部为阻塞式调用（v0.1）；recv 会阻塞直到有数据或对端关闭。
//   - 失败约定：net_connect/net_listen/net_accept 返回 -1；
//     net_send 返回已发送字节数（-1 失败）；net_recv 返回收到的字节文本，
//     对端关闭或出错返回空字符串；net_close 成功 0、失败 -1。
//   - host 支持域名与 IP（getaddrinfo 解析）。
// ===========================================================================

/// 建立到 host:port 的 TCP 连接，返回 socket；失败返回 -1。
export extern c function net_connect(host: string, port: int): int;

/// 发送字节（可能部分发送）；返回实际发送字节数，失败返回 -1。
export extern c function net_send(fd: int, data: string): int;

/// 接收最多 max_bytes 字节；返回收到的文本，EOF/失败返回空字符串。
export extern c function net_recv(fd: int, max_bytes: int): string;

/// 关闭 socket；成功返回 0，失败返回 -1。
export extern c function net_close(fd: int): int;

/// 在本地所有网卡上监听 port（0 表示由系统分配）；返回监听 socket，失败返回 -1。
export extern c function net_listen(port: int): int;

/// 接受一个连接，返回对端 socket；失败返回 -1。
export extern c function net_accept(fd: int): int;

/// 监听 socket 实际绑定的端口号（net_listen(0) 后用）；失败返回 -1。
export extern c function net_port(fd: int): int;
