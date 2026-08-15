// ===========================================================================
// std/url —— URL 解析与查询参数
//
// 用法：
//   import { url_parse, url_query, url_build_query,
//            url_scheme, url_host, url_port, url_path, url_query_part } from "std/url";
//   const u = url_parse("http://example.com:8080/a/b?x=1&y=2");
//   url_scheme(u) == "http"     url_host(u) == "example.com"
//   url_port(u) == 8080         url_path(u) == "/a/b"
//   url_query_part(u) == "x=1&y=2"
//   const q = url_query("a=1&b=2");     // map
//   (map_get(q, "a") ?? "") == "1"
//   url_build_query(q) == "a=1&b=2"
//
// url_parse 返回 map：scheme / host / port(int) / path / query。
// ===========================================================================

import { map_get, map_get_int } from "std/map";

/// 解析 URL 为 map：scheme / host / port(int) / path / query。
export extern c function url_parse(url: string): ptr<void>;

/// 解析查询字符串 "a=1&b=2" 为 map（string 值）。
export extern c function url_query(query: string): ptr<void>;

/// 把 map 序列化为查询字符串（URL 编码键与值）。
export extern c function url_build_query(map: ptr<void>): string;

/// 取协议（便捷包装）。
export function url_scheme(url: ptr<void>): string {
    return map_get(url, "scheme") ?? "";
}

/// 取主机名（便捷包装）。
export function url_host(url: ptr<void>): string {
    return map_get(url, "host") ?? "";
}

/// 取端口号（便捷包装，缺省 80）。
export function url_port(url: ptr<void>): int {
    return map_get_int(url, "port", 80);
}

/// 取路径（便捷包装）。
export function url_path(url: ptr<void>): string {
    return map_get(url, "path") ?? "";
}

/// 取查询字符串（便捷包装，不含 '?'）。
export function url_query_part(url: ptr<void>): string {
    return map_get(url, "query") ?? "";
}

/// 从完整 URL 直接解析查询参数为 map（string 值，自动 URL 解码）。
/// 示例：url_query_map("http://host/api?a=1&b=2") 后 map_get(q, "a") == "1"。
export function url_query_map(url: string): ptr<void> {
    return url_query(url_query_part(url_parse(url)));
}

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 解析网址(url: string): ptr<void> {
    return url_parse(url);
}

export function 取网址协议(url: ptr<void>): string {
    return url_scheme(url);
}

export function 取网址主机(url: ptr<void>): string {
    return url_host(url);
}

export function 取网址端口(url: ptr<void>): int {
    return url_port(url);
}

export function 取网址路径(url: ptr<void>): string {
    return url_path(url);
}

export function 取网址查询(url: ptr<void>): string {
    return url_query_part(url);
}

export function 解析查询参数(query: string): ptr<void> {
    return url_query(query);
}

export function 网址查询参数(url: string): ptr<void> {
    return url_query_map(url);
}

export function 生成查询参数(map: ptr<void>): string {
    return url_build_query(map);
}
