import { println } from "std/io";

function risky(flag: bool): int {
    if (flag) {
        throw "boom";
    }
    return 42;
}

function main(): int {
    let result = 0;
    try {
        result = risky(true);
    } catch (e: string) {
        result = 1;
    }
    try {
        result = risky(false);
    } finally {
        println("finally-ok");
    }
    // catch (e) 无类型注解：捕获所有异常，e 推断为 string 可直接使用
    // （bug #2 修复：此前 e 是 Unknown 类型，插值/比较全部报错）。
    let message = "";
    try {
        throw "no-annotation";
    } catch (e) {
        message = e;
        if (e != "no-annotation" || e.length == 0) {
            return 1;
        }
    }
    if (message != "no-annotation") {
        return 1;
    }
    return result;
}
