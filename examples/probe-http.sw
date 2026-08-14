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
    // 用 httpbin.org 验证 GET/POST（仅 http://，非 https）。
    let ok = 1;

    const get_resp = http_get("http://httpbin.org/get?x=1");
    const get_status = http_status(get_resp);
    const get_body = http_body(get_resp);
    println(`get_status=${get_status} body_len=${get_body.length}`);
    if (get_status != 200 || get_body.length == 0) {
        ok = 0;
    }

    const post_resp = http_post("http://httpbin.org/post", "name=sw&year=2026");
    const post_status = http_status(post_resp);
    const post_body = http_body(post_resp);
    println(`post_status=${post_status} body_len=${post_body.length}`);
    if (post_status != 200 || post_body.length == 0) {
        ok = 0;
    }

    const cn_resp = 取网页("http://httpbin.org/get");
    println(`cn_status=${取状态码(cn_resp)} cn_body_len=${取响应内容(cn_resp).length}`);
    if (取状态码(cn_resp) != 200) {
        ok = 0;
    }

    println(`ok=${ok}`);
    return ok == 1 ? 0 : 1;
}
