import { println } from "std/io";

class Animal {
    name: string;
    constructor(n: string) { this.name = n; }
    speak(): string { return "?"; }
    legs(): int { return 4; }
    hello(): string { return "base-hello"; }
    greet(p: string): string { return `${this.name} hi ${p}`; }
    greet(n: int): string { return `${this.name} num ${n}`; }
}

class Dog extends Animal {
    constructor(n: string) { super(n); }
    speak(): string { return "woof"; }
    greet(p: string): string { return `${this.name} bark ${p}`; }
    call_base(): string { return super.hello(); }
}

class Cat extends Animal {
    constructor(n: string) { super(n); }
    speak(): string { return "meow"; }
    legs(): int { return 0; }
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

    // 1) 基类引用虚派发
    const a: Animal = new Dog("d");
    passed = passed & check(a.speak() == "woof", "base ref dispatch dog");
    const c: Animal = new Cat("c");
    passed = passed & check(c.speak() == "meow", "base ref dispatch cat");
    passed = passed & check(c.legs() == 0, "base ref override legs");
    passed = passed & check(a.legs() == 4, "base ref inherit legs");

    // 2) 类继承多态数组
    const animals: Animal[] = [new Dog("d"), new Cat("c")];
    passed = passed & check(animals.length == 2, "inherit array length");
    passed = passed & check(animals[0].speak() == "woof", "inherit array idx0");
    passed = passed & check(animals[1].speak() == "meow", "inherit array idx1");

    // 3) for-of 类继承数组
    let buf = "";
    for (const an of animals) {
        buf = buf + an.speak() + ";";
    }
    passed = passed & check(buf == "woof;meow;", "inherit array for-of");

    // 4) 继承数组作函数参数（Dog[] → Animal[] 参数）
    const dogs: Dog[] = [new Dog("x"), new Dog("y")];
    passed = passed & check(count_speaks(dogs) == 2, "Dog[] as Animal[] param");

    // 5) 重载虚派发：基类引用调不同签名各走各槽
    const base: Animal = new Dog("d");
    passed = passed & check(base.greet("p") == "d bark p", "virtual overload string");
    passed = passed & check(base.greet(7) == "d num 7", "virtual overload int");

    // 6) super.method() 仍直调基类实现
    const dog = new Dog("d");
    passed = passed & check(dog.call_base() == "base-hello", "super.method direct");

    // 7) 静态方法/字段继承
    passed = passed & check(Derived.helper() == "base-helper", "static method inherit");
    passed = passed & check(Derived.VERSION == 3, "static field inherit");
    passed = passed & check(Base.helper() == "base-helper", "static method direct");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}

function count_speaks(list: Animal[]): int {
    let n = 0;
    for (const an of list) {
        if (an.speak() != "") { n = n + 1; }
    }
    return n;
}

class Base {
    static VERSION: int = 3;
    static helper(): string { return "base-helper"; }
}

class Derived extends Base {
    hello(): string { return "derived-hello"; }
}
