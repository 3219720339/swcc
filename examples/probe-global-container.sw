// 数组/map 全局变量（运行时 sw_global_init 构造）。
// 覆盖：const/let 数组全局（int/string/u8）、push 修改、跨模块引用、
// map 全局（map_new()）、跨模块 map 共享。
import { println } from "std/io";
import { map_new, map_set, map_get, map_has, map_keys } from "std/map";
import { CONFIG_NUMS, CONFIG_NAMES, COUNTERS, get_nums, setup_config, get_config_name } from "./lib-global-container";

const NUMS = [1, 2, 3];
const NAMES = ["a", "b", "c"];
const BYTES = [10u8, 20u8, 30u8];
const FLTS = [1.5, 2.5];
let MUT = [10, 20];
const CONFIG = map_new();

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
    // 数组全局：int
    passed = passed & check(NUMS.length == 3 && NUMS[0] == 1 && NUMS[2] == 3, "const int array global");
    // 字符串数组
    passed = passed & check(NAMES.length == 3 && NAMES[1] == "b", "const string array global");
    // u8 数组（紧凑布局）
    passed = passed & check(BYTES.length == 3 && BYTES[0] == 10 && BYTES[2] == 30, "const u8 array global");
    // float 数组
    passed = passed & check(FLTS.length == 2 && FLTS[0] == 1.5 && FLTS[1] == 2.5, "const float array global");
    // let 数组 + push
    passed = passed & check(MUT.length == 2 && MUT[0] == 10, "let array global initial");
    MUT.push(30);
    passed = passed & check(MUT.length == 3 && MUT[2] == 30, "let array global push");

    // map 全局
    map_set(CONFIG, "name", "swc");
    map_set(CONFIG, "ver", "1.0");
    passed = passed & check((map_get(CONFIG, "name") ?? "") == "swc", "map global set/get");
    passed = passed & check(map_has(CONFIG, "ver"), "map global has");
    passed = passed & check(map_keys(CONFIG).length == 2, "map global keys");

    // 跨模块数组/map
    passed = passed & check(CONFIG_NUMS.length == 3 && CONFIG_NUMS[1] == 8, "cross-module array global");
    passed = passed & check(CONFIG_NAMES.length == 2 && CONFIG_NAMES[0] == "x", "cross-module string array");
    passed = passed & check(COUNTERS.length == 1 && COUNTERS[0] == 5, "cross-module let array");
    passed = passed & check(get_nums()[0] == 7, "cross-module fn returns array global");
    setup_config();
    passed = passed & check(get_config_name() == "shared", "cross-module map global");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
