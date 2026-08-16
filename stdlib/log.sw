// std/log - 适合 CLI/服务端工具的 stderr 日志。

import { eprintln } from "std/io";
import { datetime_string_ms, now_ms } from "std/time";
import { map_keys, map_get } from "std/map";
import { json_object_new, json_object_set, json_string_new, json_stringify } from "std/json";

export const LOG_DEBUG = 10;
export const LOG_INFO = 20;
export const LOG_WARN = 30;
export const LOG_ERROR = 40;

extern c function log_runtime_set_level(level: int): void;
extern c function log_runtime_level(): int;

export function log_set_level(level: int): void { log_runtime_set_level(level); }
export function log_level(): int { return log_runtime_level(); }

function level_name(level: int): string {
    return level <= LOG_DEBUG ? "DEBUG" : (level <= LOG_INFO ? "INFO" : (level <= LOG_WARN ? "WARN" : "ERROR"));
}

export function log_write(level: int, message: string): void {
    if (level < log_level()) { return; }
    eprintln(`${datetime_string_ms(now_ms())} ${level_name(level)} ${message}`);
}
export function debug(message: string): void { log_write(LOG_DEBUG, message); }
export function info(message: string): void { log_write(LOG_INFO, message); }
export function warn(message: string): void { log_write(LOG_WARN, message); }
export function error(message: string): void { log_write(LOG_ERROR, message); }

/// 结构化 JSON 日志。fields 为 string map，可供日志采集器直接读取。
export function log_json(level: int, message: string, fields: ptr<void>): void {
    if (level < log_level()) { return; }
    const doc = json_object_new();
    json_object_set(doc, "time", json_string_new(datetime_string_ms(now_ms())));
    json_object_set(doc, "level", json_string_new(level_name(level)));
    json_object_set(doc, "message", json_string_new(message));
    for (const key of map_keys(fields)) {
        json_object_set(doc, key, json_string_new(map_get(fields, key) ?? ""));
    }
    eprintln(json_stringify(doc));
}
