import { println } from "std/io";
import { http_open, http_request_on, http_close, http_get_on, http_status, http_body } from "std/http";
import { regex_split, regex_escape, regex_captures, regex_match } from "std/regex";
import { word_wrap, truncate_middle } from "std/string";
import { map_get_int, map_new, map_set } from "std/map";

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

    // ---------- regex 增强 ----------
    const parts = regex_split("a,b;c", "[,;]");
    passed = passed & check(parts.length == 3 && parts[0] == "a" && parts[2] == "c", "regex_split");

    const escaped = regex_escape("a.b*c");
    passed = passed & check(escaped == "a\\.b\\*c", "regex_escape text");
    passed = passed & check(regex_match("a.b*c", escaped), "regex_escape literal match");

    const caps = regex_captures("2026-08-15", "(\\d+)-(\\d+)-(\\d+)");
    passed = passed & check(
        caps.length == 4 && caps[0] == "2026-08-15" && caps[1] == "2026" && caps[2] == "08" && caps[3] == "15",
        "regex_captures groups"
    );
    passed = passed & check(regex_split("no-separator-here", ",").length == 1, "regex_split no match");
    const no_caps = regex_captures("abc", "(\\d+)");
    passed = passed & check(
        no_caps.length == 2 && no_caps[0] == "" && no_caps[1] == "",
        "regex_captures no match"
    );

    // ---------- string 实用 ----------
    const wrapped = word_wrap("hello world swcc lang", 10);
    passed = passed & check(
        wrapped.length == 3 && wrapped[0] == "hello" && wrapped[1] == "world swcc" && wrapped[2] == "lang",
        "word_wrap"
    );
    const wrapped2 = word_wrap("a b c", 5);
    passed = passed & check(wrapped2.length == 1 && wrapped2[0] == "a b c", "word_wrap short");
    passed = passed & check(truncate_middle("abcdefghij", 7) == "ab...ij", "truncate_middle");
    passed = passed & check(truncate_middle("abcdef", 5) == "a...f", "truncate_middle 5");
    passed = passed & check(truncate_middle("abc", 5) == "abc", "truncate_middle short");
    passed = passed & check(truncate_middle("你好世界abc", 6) == "你...bc", "truncate_middle UTF-8");

    // ---------- HTTP keep-alive 会话（httpbin 双请求复用同一连接） ----------
    const session = http_open("httpbin.org", 80);
    passed = passed & check(session >= 0, "http_open");

    const r1 = http_request_on(session, "GET", "/get", null, "");
    const s1 = map_get_int(r1, "status", 0);
    const b1 = http_body(r1);
    passed = passed & check(s1 == 200 && b1.length > 0, "http_request_on #1 (new connection)");

    // 同一会话第二次请求：连接复用
    const r2 = http_get_on(session, "/get?x=2");
    const s2 = map_get_int(r2, "status", 0);
    const b2 = http_body(r2);
    passed = passed & check(s2 == 200 && b2.length > 0, "http_request_on #2 (keep-alive reuse)");

    // 自定义请求头
    const headers = map_new();
    map_set(headers, "User-Agent", "sw-test/1.0");
    const r3 = http_request_on(session, "GET", "/get", headers, "");
    passed = passed & check(map_get_int(r3, "status", 0) == 200, "http_request_on #3 with headers");

    passed = passed & check(http_close(session) >= 0, "http_close");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
