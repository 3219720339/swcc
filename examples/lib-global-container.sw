// 跨模块数组/map 全局定义模块（被 probe-global-container.sw 引用）。
import { map_new, map_set, map_get } from "std/map";

export const CONFIG_NUMS = [7, 8, 9];
export const CONFIG_NAMES = ["x", "y"];
export let COUNTERS = [5];
export const SHARED_CONFIG = map_new();

export function get_nums(): int[] {
    return CONFIG_NUMS;
}

export function setup_config(): void {
    map_set(SHARED_CONFIG, "name", "shared");
}

export function get_config_name(): string {
    return map_get(SHARED_CONFIG, "name") ?? "";
}
