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
