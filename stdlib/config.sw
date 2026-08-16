// std/config - dotenv、环境变量与 TOML/JSON 配置组合。

import { read_all } from "std/fs";
import { split_once, trim, starts_with, ends_with, substring } from "std/string";
import { map_new, map_set, map_get, map_keys, map_merge } from "std/map";
import { getenv } from "std/os";
import { toml_parse } from "std/toml";
import { json_parse, json_kind, json_object_keys, json_object_get, json_string, json_int, json_float, json_bool } from "std/json";

/// 读取 dotenv 文本。支持空行、# 注释、export KEY=value 和单双引号值。
export function config_parse_dotenv(text: string): ptr<void> {
    const result = map_new();
    for (const raw of text.lines()) {
        let line = trim(raw);
        if (line == "" || starts_with(line, "#")) { continue; }
        if (starts_with(line, "export ")) { line = trim(substring(line, 7, line.length - 7)); }
        const pair = split_once(line, "=");
        if (pair.length != 2) { continue; }
        const key = trim(pair[0]);
        let value = trim(pair[1]);
        if (value.length >= 2 && ((starts_with(value, "\"") && ends_with(value, "\"")) || (starts_with(value, "'") && ends_with(value, "'")))) {
            value = substring(value, 1, value.length - 2);
        }
        if (key != "") { map_set(result, key, value); }
    }
    return result;
}

export function config_load_dotenv(path: string): ptr<void> { return config_parse_dotenv(read_all(path)); }
export function config_load_toml(path: string): ptr<void> { return toml_parse(read_all(path)); }

/// 把所有已存在的环境变量覆盖到配置；prefix 为空时读取全部键。
export function config_apply_env(config: ptr<void>, prefix: string): ptr<void> {
    const result = map_merge(config, map_new());
    for (const key of map_keys(config)) {
        const env_name = prefix + key;
        const value = getenv(env_name);
        if (value != null) { map_set(result, key, value); }
    }
    return result;
}

/// 把 JSON 对象的一层标量字段转换为 string map；非标量用 JSON 文本表示。
export function config_json_object(text: string): ptr<void> {
    const result = map_new();
    const doc = json_parse(text);
    if (json_kind(doc) != 6) { return result; }
    for (const key of json_object_keys(doc)) {
        const value = json_object_get(doc, key);
        const kind = json_kind(value);
        if (kind == 4) { map_set(result, key, json_string(value)); }
        else if (kind == 2) { map_set(result, key, `${json_int(value)}`); }
        else if (kind == 3) { map_set(result, key, `${json_float(value)}`); }
        else if (kind == 1) { map_set(result, key, json_bool(value) == 1 ? "true" : "false"); }
    }
    return result;
}

export function config_get(config: ptr<void>, key: string, fallback: string): string { return map_get(config, key) ?? fallback; }
