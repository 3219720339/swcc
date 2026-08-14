// lib-std.sw：跨文件调用演示——本模块内部全部使用标准库，
// 供 probe-cross-stdlib.sw 调用。
import { to_upper, reverse, format, join, split, format_float } from "std/string";
import { gcd, sqrt, pi, rand_int_range } from "std/math";
import { datetime_string, date_string, now_sec } from "std/time";
import { sort_int, sum_int, unique_string } from "std/array";
import { map_new, map_set, map_get, map_set_int, map_get_int, map_inc } from "std/map";
import { json_parse, json_string, json_int, json_object_get } from "std/json";
import { md5, sha256 } from "std/hash";
import { regex_find, regex_find_all } from "std/regex";
import { read_lines, write_lines, path_join, exists } from "std/fs";
import { base64_encode, hex_encode } from "std/encoding";

export function describe(name: string): string {
    return `${to_upper(name)}-${reverse(name)}`;
}

export function math_report(): string {
    return format("gcd=%d sqrt=%.0f pi=%.2f", gcd(12, 18), sqrt(16.0), pi());
}

export function today(): string {
    return date_string(now_sec());
}

export function now_text(): string {
    return datetime_string(now_sec());
}

export function array_sum(nums: int[]): int {
    sort_int(nums);
    return sum_int(nums);
}

export function unique_words(words: string[]): string {
    return join(unique_string(words), ",");
}

export function counter_report(): string {
    const m = map_new();
    map_set(m, "name", "sw");
    map_set_int(m, "count", 1);
    map_inc(m, "count", 4);
    return format("name=%s count=%d", map_get(m, "name") ?? "", map_get_int(m, "count", 0));
}

export function json_report(): string {
    const doc = json_parse(`{"lang":"sw","year":2026}`);
    return format("lang=%s year=%d", json_string(json_object_get(doc, "lang")), json_int(json_object_get(doc, "year")));
}

export function hash_report(): string {
    return format("md5=%s sha256_len=%d", md5("hello"), sha256("hello").length);
}

export function regex_report(text: string): string {
    const all = regex_find_all(text, "\\d+");
    return format("first=%s all=%d", regex_find(text, "\\d+"), all.length);
}

export function file_report(base: string): string {
    const path = path_join(base, "cross-lib.txt");
    write_lines(path, ["one", "two", "three"]);
    const lines = read_lines(path);
    return format("exists=%d lines=%d first=%s", exists(path), lines.length, lines[0]);
}

export function encode_report(): string {
    return format("b64=%s hex=%s", base64_encode("hi"), hex_encode("hi"));
}

export function random_report(): string {
    const v = rand_int_range(1, 101);
    return format("rand=%d", v);
}
