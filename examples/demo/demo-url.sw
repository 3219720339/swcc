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
import { map_get, map_new, map_set } from "std/map";

function main(): int {
    const u = url_parse("http://example.com:8080/a/b?x=1&y=2");
    println(url_scheme(u));
    println(url_host(u));
    println(url_port(u));
    println(url_path(u));
    println(url_query_part(u));

    const u2 = 解析网址("https://api.test.com/data");
    println(取网址协议(u2));
    println(取网址主机(u2));
    println(取网址端口(u2));
    println(取网址路径(u2));

    const q = url_query("a=1&b=hello&empty=");
    println(map_get(q, "a") ?? "(无)");
    println(map_get(q, "b") ?? "(无)");
    println(map_get(q, "empty") ?? "(无)");
    println(map_get(q, "missing") ?? "(无)");

    const m = map_new();
    map_set(m, "name", "sw");
    map_set(m, "year", "2026");
    println(url_build_query(m));
    println(生成查询参数(m));

    const cq = 解析查询参数("x=1&y=2");
    println(map_get(cq, "x") ?? "(无)");
    println(map_get(cq, "y") ?? "(无)");
    return 0;
}
