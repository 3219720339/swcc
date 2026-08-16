// ===========================================================================
// std/toml —— TOML 配置解析（最小实现）
//
// 用法：
//   import { toml_parse, toml_get, toml_set } from "std/toml";
//   const cfg = toml_parse("[server]\nport = 8080\nhost = \"127.0.0.1\"\n");
//   (map_get(cfg, "server.port") ?? "") == "8080"
//
// 支持：[section]、key = value（字符串去引号、数字/布尔/数组按文本存）、
// # 注释、空行；[section] 下的键存为 "section.key"。
// ===========================================================================

import { map_get, map_set, map_keys } from "std/map";
import { read_all, atomic_write } from "std/fs";
import { escape } from "std/string";

/// 解析 TOML 文本为 map（键 "section.key" 或 "key"，值为 string）。
export extern c function toml_parse(text: string): ptr<void>;

/// 读取配置值（便捷包装，键含节名如 "server.port"）。
export function toml_get(cfg: ptr<void>, key: string): string? {
    return map_get(cfg, key);
}

/// 写入配置值（便捷包装）；成功返回 0，失败返回 -1。
export function toml_set(cfg: ptr<void>, key: string, value: string): int {
    return map_set(cfg, key, value);
}

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 解析TOML(text: string): ptr<void> {
    return toml_parse(text);
}

export function 取TOML项(cfg: ptr<void>, key: string): string? {
    return toml_get(cfg, key);
}

export function 置TOML项(cfg: ptr<void>, key: string, value: string): int {
    return toml_set(cfg, key, value);
}

/// 读取 TOML 文件；不存在/空文件返回空 map。
export function toml_read_file(path: string): ptr<void> {
    return toml_parse(read_all(path));
}

/// 将扁平 string map 原子写入 TOML。键保持原样（可含 section.key），值统一双引号转义。
export function toml_write_file(path: string, cfg: ptr<void>): int {
    let text = "";
    for (const key of map_keys(cfg)) {
        text = text + key + " = \"" + escape(map_get(cfg, key) ?? "") + "\"\n";
    }
    return atomic_write(path, text);
}
