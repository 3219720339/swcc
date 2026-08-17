import { println } from "std/io";
import { Animal } from "./lib-cross-animal";

class Dog extends Animal {
    constructor(n: string) { super(n); }
    speak(): string { return "woof"; }
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
    // 跨模块继承虚派发：Dog 的 vtable（main 模块生成）需引用 lib 模块的
    // Animal.speak/legs/hello（未覆盖的走 import 兜底填槽）。
    const a: Animal = new Dog("d");
    passed = passed & check(a.speak() == "woof", "cross-module virtual override");
    passed = passed & check(a.legs() == 4, "cross-module inherited method");
    passed = passed & check(a.hello() == "animal-hello", "cross-module base method");
    const arr: Animal[] = [new Dog("x"), new Dog("y")];
    passed = passed & check(arr[0].speak() == "woof", "cross-module array dispatch");
    passed = passed & check(arr[1].legs() == 4, "cross-module array inherited");
    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
