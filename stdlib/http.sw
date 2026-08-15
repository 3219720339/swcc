// ===========================================================================
// std/http —— HTTP 客户端（阻塞式，基于 std/net 的 TCP）
//
// 用法：
//   import { http_get, http_post, http_status, http_body } from "std/http";
//   import { map_get, map_get_int } from "std/map";
//   const resp = http_get("http://example.com/");
//   const status = map_get_int(resp, "status", 0);
//   const body = map_get(resp, "body") ?? "";
//
// 返回值为 map：status(int) / body(string) / headers(string)。
// 也提供便捷包装 http_status(resp) / http_body(resp)。
// 说明：v0.1 仅支持 http://（明文），https/TLS 留待后续。
// ===========================================================================

import { map_get, map_get_int } from "std/map";
import { json_parse } from "std/json";

/// 发起 GET 请求，返回 map：status / body / headers。
/// 失败时 status 为 0、body 为空串。
export extern c function http_get(url: string): ptr<void>;

/// 发起 POST 请求（Content-Type: application/x-www-form-urlencoded），
/// 返回 map：status / body / headers。
export extern c function http_post(url: string, body: string): ptr<void>;

/// 取响应状态码（便捷包装）。
export function http_status(resp: ptr<void>): int {
    return map_get_int(resp, "status", 0);
}

/// 取响应体文本（便捷包装）。
export function http_body(resp: ptr<void>): string {
    return map_get(resp, "body") ?? "";
}

/// 取响应头原始文本（便捷包装）。
export function http_headers(resp: ptr<void>): string {
    return map_get(resp, "headers") ?? "";
}

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 取网页(url: string): ptr<void> {
    return http_get(url);
}

export function 网页POST(url: string, body: string): ptr<void> {
    return http_post(url, body);
}

export function 取状态码(resp: ptr<void>): int {
    return http_status(resp);
}

export function 取响应内容(resp: ptr<void>): string {
    return http_body(resp);
}

/// GET 并把响应体解析为 JSON 值（复用 json_parse）；请求失败或 JSON 非法
/// 返回 null（kind 0）。示例：http_get_json("http://host/api")。
export function http_get_json(url: string): ptr<void> {
    const resp = http_get(url);
    return json_parse(http_body(resp));
}

export function 取网页JSON(url: string): ptr<void> {
    return http_get_json(url);
}

/// 建立 HTTP keep-alive 会话（同一连接复用多次请求）；失败返回 -1。
/// 与 http_request_on/http_close 配合：会话内每次请求复用连接，
/// 响应按 Content-Length 分帧；服务器关闭连接时请求返回 status 0，
/// 需重新 http_open。
export extern c function http_open(host: string, port: int): int;

/// 在会话上发请求（method/path/headers(map，可 null)/body），返回
/// map：status/body/headers。连接保持复用。
export extern c function http_request_on(
    handle: int, method: string, path: string, headers: ptr<void>, body: string
): ptr<void>;

/// 关闭 keep-alive 会话。
export extern c function http_close(handle: int): int;

/// 会话 GET（便捷包装）：http_get_on(handle, "/path")。
export function http_get_on(handle: int, path: string): ptr<void> {
    return http_request_on(handle, "GET", path, null, "");
}

/// 会话 POST（便捷包装）：http_post_on(handle, "/path", body)。
export function http_post_on(handle: int, path: string, body: string): ptr<void> {
    return http_request_on(handle, "POST", path, null, body);
}

export function 打开HTTP会话(host: string, port: int): int {
    return http_open(host, port);
}

export function HTTP会话请求(
    handle: int, method: string, path: string, headers: ptr<void>, body: string
): ptr<void> {
    return http_request_on(handle, method, path, headers, body);
}

export function 关闭HTTP会话(handle: int): int {
    return http_close(handle);
}

export function HTTP会话GET(handle: int, path: string): ptr<void> {
    return http_get_on(handle, path);
}

export function HTTP会话POST(handle: int, path: string, body: string): ptr<void> {
    return http_post_on(handle, path, body);
}
