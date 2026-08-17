// 字符串全局变量运行时初始化（数据段无法静态表示 sw_string 指针，
// 由 sw_global_init 在 main 之前构造）。覆盖 const/let 全局、static 字段
// 与跨模块引用/修改。
import { println } from "std/io";
import { APP_NAME, version, get_app, get_version } from "./lib-string-global";

const GREETING = "hello world";
let counter_name = "swc";
class Config {
    static NAME: string = "swc-config";
    static TAG: string = "v1";
}

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
    passed = passed & check(GREETING == "hello world", "const string global");
    passed = passed & check(GREETING.length == 11, "const string global length");
    passed = passed & check(counter_name == "swc", "let string global initial");
    counter_name = "swc-v2";
    passed = passed & check(counter_name == "swc-v2", "let string global reassign");
    passed = passed & check(Config.NAME == "swc-config", "static string field");
    passed = passed & check(Config.TAG == "v1", "static string field 2");
    const combined = GREETING + "!";
    passed = passed & check(combined == "hello world!", "string global concat");

    // 跨模块字符串全局：定义在 lib-string-global.sw，多个模块引用。
    passed = passed & check(APP_NAME == "swc-app", "cross-module const string global");
    passed = passed & check(get_app() == "swc-app", "cross-module function reads global");
    passed = passed & check(version == "v1.0", "cross-module let string global");
    passed = passed & check(get_version() == "v1.0", "cross-module fn reads let global");
    // 跨模块修改：main 写 version，定义模块函数读到新值（共享同一份数据）。
    version = "v2.0";
    passed = passed & check(version == "v2.0", "cross-module let reassign");
    passed = passed & check(get_version() == "v2.0", "cross-module fn sees mutation");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
