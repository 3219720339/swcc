import { println, print, flush } from "std/io";
import { http_open, http_request_on, http_close, http_status, http_body } from "std/http";
import { regex_split, regex_escape, regex_captures, regex_match } from "std/regex";
import { word_wrap, truncate_middle } from "std/string";
import { map_get_int } from "std/map";
import { format } from "std/string";

// HTTP keep-alive 会话 + regex 增强 + 文本实用演示。
function main(): int {
    println("== HTTP keep-alive 会话（同一连接多次请求） ==");
    const session = http_open("httpbin.org", 80);
    if (session >= 0) {
        const r1 = http_request_on(session, "GET", "/get", null, "");
        println(format("请求1: status=%d body=%d 字节", map_get_int(r1, "status", 0), http_body(r1).length));
        const r2 = http_request_on(session, "GET", "/get?x=2", null, "");
        println(format("请求2（连接复用）: status=%d body=%d 字节", map_get_int(r2, "status", 0), http_body(r2).length));
        http_close(session);
    } else {
        println("连接失败（离线环境跳过）");
    }

    println("== regex 增强 ==");
    const parts = regex_split("订单-2026,状态:已发", "[-,:]");
    print(format("拆分=%s", join(parts)));
    println("");
    const safe = regex_escape("a.b*c?");
    println(format("转义=%s  字面匹配=%s", safe, regex_match("a.b*c?", safe) ? "是" : "否"));
    const caps = regex_captures("2026-08-15", "(\\d+)-(\\d+)-(\\d+)");
    println(format("捕获组: 年=%s 月=%s 日=%s", caps[1], caps[2], caps[3]));

    println("== 文本实用 ==");
    const lines = word_wrap("Sw 是一门静态强类型面向本机程序的编译型语言", 14);
    for (const line of lines) {
        println(line);
    }
    println(format("文件名=%s", truncate_middle("非常长的一个示例文件名用于演示中间截断效果.sw", 16)));
    flush();
    return 0;
}

function join(items: string[]): string {
    let result = "";
    for (const item of items) {
        if (result != "") {
            result = result + " ";
        }
        result = result + item;
    }
    return result;
}
