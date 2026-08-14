import { println } from "std/io";
import {
    url_parse,
    url_query,
    url_build_query,
    url_scheme,
    url_host,
    url_port,
    url_path,
    url_query_part,
    解析网址,
    取网址协议,
    取网址主机,
    取网址端口,
    取网址路径,
    取网址查询,
    解析查询参数,
    生成查询参数,
} from "std/url";
import { map_get, map_get_int, map_new, map_set } from "std/map";

function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

function main(): int {
    let passed = 1;

    // 完整 URL
    const u = url_parse("http://example.com:8080/a/b?x=1&y=2");
    passed = passed & check(url_scheme(u) == "http", "scheme");
    passed = passed & check(url_host(u) == "example.com", "host");
    passed = passed & check(url_port(u) == 8080, "port");
    passed = passed & check(url_path(u) == "/a/b", "path");
    passed = passed & check(url_query_part(u) == "x=1&y=2", "query part");

    // 默认端口（无端口号）
    const u2 = url_parse("https://api.test.com/data");
    passed = passed & check(url_scheme(u2) == "https", "scheme https");
    passed = passed & check(url_host(u2) == "api.test.com", "host no port");
    passed = passed & check(url_port(u2) == 443, "default https port");
    passed = passed & check(url_path(u2) == "/data", "path no query");

    // 无路径
    const u3 = url_parse("http://localhost:3000");
    passed = passed & check(url_host(u3) == "localhost", "host only");
    passed = passed & check(url_port(u3) == 3000, "port only");
    passed = passed & check(url_path(u3) == "/", "default path slash");

    // map 形式
    passed = passed & check(map_get_int(u, "port", -1) == 8080, "map port int");
    passed = passed & check((map_get(u, "scheme") ?? "") == "http", "map scheme");

    // 查询参数解析
    const q = url_query("a=1&b=hello&empty=");
    passed = passed & check((map_get(q, "a") ?? "") == "1", "query a");
    passed = passed & check((map_get(q, "b") ?? "") == "hello", "query b");
    passed = passed & check((map_get(q, "empty") ?? "") == "", "query empty");
    passed = passed & check((map_get(q, "missing") ?? "none") == "none", "query missing");

    // 生成查询参数
    const m = map_new();
    map_set(m, "name", "sw");
    map_set(m, "year", "2026");
    passed = passed & check(url_build_query(m) == "name=sw&year=2026", "build query");
    passed = passed & check(生成查询参数(m) == "name=sw&year=2026", "cn build query");

    // 中文名
    const cu = 解析网址("http://cn.example.com:81/x?q=1");
    passed = passed & check(取网址协议(cu) == "http", "cn scheme");
    passed = passed & check(取网址主机(cu) == "cn.example.com", "cn host");
    passed = passed & check(取网址端口(cu) == 81, "cn port");
    passed = passed & check(取网址路径(cu) == "/x", "cn path");
    passed = passed & check(取网址查询(cu) == "q=1", "cn query");
    const cq = 解析查询参数("x=1&y=2");
    passed = passed & check((map_get(cq, "y") ?? "") == "2", "cn query map");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
