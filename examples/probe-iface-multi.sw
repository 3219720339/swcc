import { println } from "std/io";

// 多接口实现：一个类实现多个接口
interface 会飞 {
    fly(): string;
}

interface 会游泳 {
    swim(): string;
}

class 鸭子 implements 会飞, 会游泳 {
    name: string;

    constructor(name: string) {
        this.name = name;
    }

    fly(): string {
        return this.name + " 飞";
    }

    swim(): string {
        return this.name + " 游";
    }
}

// 接口继承接口：飞行器接口继承会飞接口（新增方法）
interface 会降落 extends 会飞 {
    land(): string;
}

class 飞机 implements 会降落 {
    fly(): string {
        return "飞机飞";
    }

    land(): string {
        return "飞机降落";
    }
}

// 泛型接口 + 多实现
interface 容器<T> {
    get(): T;
    set(value: T): void;
}

interface 可计数 {
    count(): int;
}

class 计数盒<T> implements 容器<T>, 可计数 {
    value: T;
    items: int;

    constructor(value: T, items: int) {
        this.value = value;
        this.items = items;
    }

    get(): T {
        return this.value;
    }

    set(value: T): void {
        this.value = value;
    }

    count(): int {
        return this.items;
    }
}

// 类继承基类 + 多接口（混合）
class 生物 {
    name: string;

    constructor(name: string) {
        this.name = name;
    }

    info(): string {
        return "生物:" + this.name;
    }
}

class 飞鱼 extends 生物 implements 会飞, 会游泳 {
    constructor(name: string) {
        super(name);
    }

    fly(): string {
        return this.name + " 滑翔";
    }

    swim(): string {
        return this.name + " 游泳";
    }
}

function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}

function 测试会飞接口(f: 会飞): string {
    return f.fly();
}

function main(): int {
    let passed = 1;

    // 1) 多接口实现：类可赋值给任一接口，接口引用派发正确
    const duck = new 鸭子("唐老鸭");
    const f1: 会飞 = duck;
    const s1: 会游泳 = duck;
    passed = passed & check(f1.fly() == "唐老鸭 飞", "multi iface fly dispatch");
    passed = passed & check(s1.swim() == "唐老鸭 游", "multi iface swim dispatch");
    passed = passed & check(测试会飞接口(duck) == "唐老鸭 飞", "iface param dispatch");

    // 2) 接口继承接口：实现子接口的类自动满足父接口
    const plane = new 飞机();
    const f2: 会飞 = plane;  // 子接口实现可赋给父接口
    const l2: 会降落 = plane;
    passed = passed & check(f2.fly() == "飞机飞", "iface extends base dispatch");
    passed = passed & check(l2.fly() == "飞机飞", "iface extends inherited method");
    passed = passed & check(l2.land() == "飞机降落", "iface extends own method");
    const f4: 会飞 = l2;  // 子接口引用赋给父接口引用
    passed = passed & check(f4.fly() == "飞机飞", "iface ref to parent iface");

    // 3) 泛型接口多实现
    const box = new 计数盒<int>(42, 7);
    const c1: 容器<int> = box;
    const c2: 可计数 = box;
    passed = passed & check(c1.get() == 42, "generic iface get");
    c1.set(99);
    passed = passed & check(box.get() == 99, "generic iface set through iface");
    passed = passed & check(c2.count() == 7, "generic multi iface count");

    // 4) 继承 + 多接口混合
    const fish = new 飞鱼("飞鱼");
    const b1: 生物 = fish;
    const f3: 会飞 = fish;
    const s3: 会游泳 = fish;
    passed = passed & check(b1.info() == "生物:飞鱼", "extends base method");
    passed = passed & check(f3.fly() == "飞鱼 滑翔", "extends + iface fly");
    passed = passed & check(s3.swim() == "飞鱼 游泳", "extends + iface swim");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
