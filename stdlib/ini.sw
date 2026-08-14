// ===========================================================================
// std/ini —— INI 配置解析
//
// 用法：
//   import { ini_parse, ini_save, ini_get, ini_set } from "std/ini";
//   const cfg = ini_parse("[server]\nport = 8080\nhost = 127.0.0.1\n");
//   (map_get(cfg, "server.port") ?? "") == "8080"
//   ini_save(cfg)  // 序列化回 INI 文本
//
// 规则：支持 [section]、key = value、# 与 ; 注释、空行；
// [section] 下的键存为 "section.key"。
// ===========================================================================

import { map_get, map_set } from "std/map";

/// 解析 INI 文本为 map（键 "section.key" 或 "key"，值为 string）。
export extern c function ini_parse(text: string): ptr<void>;

/// 把 map 序列化为 INI 文本（无节键在前，[section] 分组）。
export extern c function ini_save(map: ptr<void>): string;

/// 读取配置值（便捷包装，键含节名如 "server.port"）。
export function ini_get(cfg: ptr<void>, key: string): string? {
    return map_get(cfg, key);
}

/// 写入配置值（便捷包装）；成功返回 0，失败返回 -1。
export function ini_set(cfg: ptr<void>, key: string, value: string): int {
    return map_set(cfg, key, value);
}

// ---------------------------------------------------------------------------
// 中文函数名（转发到英文实现，火山风格命名）
// ---------------------------------------------------------------------------

export function 解析配置文件(text: string): ptr<void> {
    return ini_parse(text);
}

export function 保存配置文件(cfg: ptr<void>): string {
    return ini_save(cfg);
}

export function 取配置项(cfg: ptr<void>, key: string): string? {
    return ini_get(cfg, key);
}

export function 置配置项(cfg: ptr<void>, key: string, value: string): int {
    return ini_set(cfg, key, value);
}
