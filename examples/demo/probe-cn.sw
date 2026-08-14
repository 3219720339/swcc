import { println } from "std/io";
import {
    取文本中间,
    批量取文本中间,
    取字符代码,
    连续子文本替换,
    删除前缀,
} from "std/string";

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
    const 文本 = "  Hello，火山  ";
    passed = passed & check(文本.是否空白() == false, "chain is_blank");
    passed = passed & check("  ".是否空白(), "chain blank");
    passed = passed & check(文本.删全部空白() == "Hello，火山", "chain strip");
    passed = passed & check(文本.转大写() == "  HELLO，火山  ", "chain upper");
    passed = passed & check(文本.开头为("  He"), "chain starts_with");
    passed = passed & check(取文本中间("你好，火山", "你", "火") == "好，", "fn between");
    passed = passed & check(取字符代码("你", 0) == 20320, "fn char_code");

    const 片段 = 批量取文本中间("a[1]b[22]c", "[", "]");
    passed = passed & check(片段.length == 2 && 片段[1] == "22", "fn extract batch");

    const 替换结果 = 连续子文本替换("你好，火山", "你好", "Hello", "火山", "火山中文编程");
    passed = passed & check(替换结果 == "Hello，火山中文编程", "fn replace_pairs");
    passed = passed & check("file.txt".删除前缀("file") == ".txt", "fn remove_prefix");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
