import { println } from "std/io";
import {
    http_get,
    http_post,
    http_status,
    http_body,
    取网页,
    取状态码,
    取响应内容,
} from "std/http";

function main(): int {
    const get_resp = http_get("http://httpbin.org/get?x=1");
    println(http_status(get_resp));
    println(http_body(get_resp).length);

    const post_resp = http_post("http://httpbin.org/post", "name=sw&year=2026");
    println(http_status(post_resp));
    println(http_body(post_resp).length);

    const cn_resp = 取网页("http://httpbin.org/get");
    println(取状态码(cn_resp));
    println(取响应内容(cn_resp).length);
    return 0;
}
