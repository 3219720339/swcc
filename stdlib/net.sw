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

/// 创建 UDP 数据报 socket；失败返回 -1。
export extern c function udp_socket(): int;

/// UDP 绑定本地端口（0 由系统分配）；成功返回 0，失败返回 -1。
export extern c function udp_bind(fd: int, port: int): int;

/// UDP 发送数据报到 host:port；返回发送字节数，失败返回 -1。
export extern c function udp_send(fd: int, host: string, port: int, data: string): int;

/// UDP 接收数据报；返回收到的文本，失败返回空串。
export extern c function udp_recv(fd: int, max_bytes: int): string;

/// 关闭 UDP socket；成功返回 0，失败返回 -1。
export extern c function udp_close(fd: int): int;

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 创建UDPSocket(): int {
    return udp_socket();
}

export function 绑定UDP端口(fd: int, port: int): int {
    return udp_bind(fd, port);
}

export function 发送UDP数据(fd: int, host: string, port: int, data: string): int {
    return udp_send(fd, host, port, data);
}

export function 接收UDP数据(fd: int, max_bytes: int): string {
    return udp_recv(fd, max_bytes);
}

/// 带超时建立 TCP 连接（timeout_ms 毫秒，<=0 不限时）；成功返回 socket，
/// 失败/超时返回 -1。用于避免对不可达主机无限阻塞。
export extern c function net_connect_timeout(host: string, port: int, timeout_ms: int): int;

/// 设置接收超时（毫秒，0 不限时）；成功返回 0，失败返回 -1。
/// 超时后 net_recv 返回空串。
export extern c function net_set_recv_timeout(fd: int, timeout_ms: int): int;

/// 设置发送超时（毫秒，0 不限时）；成功返回 0，失败返回 -1。
export extern c function net_set_send_timeout(fd: int, timeout_ms: int): int;

/// 当前可读取的字节数；失败返回 -1。
export extern c function net_available(fd: int): int;

/// 域名解析为 IPv4 点分字符串；失败返回空串。
export extern c function net_resolve(host: string): string;

/// 对端 IP（IPv4 点分字符串）；失败返回空串。
export extern c function net_peer_ip(fd: int): string;

/// 对端端口；失败返回 -1。
export extern c function net_peer_port(fd: int): int;

/// 启用/禁用 TCP keepalive；成功返回 0，失败返回 -1。
export extern c function net_set_keepalive(fd: int, enabled: bool): int;

/// 读取直到对端关闭（返回全部内容）；适合读 HTTP 响应体。
export extern c function net_recv_until_close(fd: int): string;

/// 完整发送（循环直到全部写入）；返回发送字节数，失败返回 -1。
export extern c function net_send_all(fd: int, data: string): int;

export function 带超时连接(host: string, port: int, timeout_ms: int): int {
    return net_connect_timeout(host, port, timeout_ms);
}

export function 设置接收超时(fd: int, timeout_ms: int): int {
    return net_set_recv_timeout(fd, timeout_ms);
}

export function 设置发送超时(fd: int, timeout_ms: int): int {
    return net_set_send_timeout(fd, timeout_ms);
}

export function 取可读字节数(fd: int): int {
    return net_available(fd);
}

export function 域名解析(host: string): string {
    return net_resolve(host);
}

export function 取对端IP(fd: int): string {
    return net_peer_ip(fd);
}

export function 取对端端口(fd: int): int {
    return net_peer_port(fd);
}

export function 设置心跳(fd: int, enabled: bool): int {
    return net_set_keepalive(fd, enabled);
}

export function 读取到关闭(fd: int): string {
    return net_recv_until_close(fd);
}

export function 完整发送(fd: int, data: string): int {
    return net_send_all(fd, data);
}
