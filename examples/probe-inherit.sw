import { println } from "std/io";

class Animal {
    name: string;

    constructor(name: string) {
        this.name = name;
    }

    speak(): string {
        return `${this.name} makes a sound`;
    }

    legs(): int {
        return 4;
    }
}

class Dog extends Animal {
    breed: string;

    constructor(name: string, breed: string) {
        super(name);
        this.breed = breed;
    }

    speak(): string {
        return `${this.name} barks`;
    }
}

function check(prev: int, cond: bool, label: string): int {
    let state = "FAIL";
    if (cond) {
        state = "ok";
    }
    println(`[${state}] ${label}`);
    if (cond) {
        return prev;
    }
    return 0;
}

function main(): int {
    let ok = 1;
    const a = new Animal("generic");
    ok = check(ok, a.name == "generic", "base_field");
    ok = check(ok, a.legs() == 4, "base_method");
    const d = new Dog("rex", "lab");
    ok = check(ok, d.name == "rex", "inherit_field");
    ok = check(ok, d.breed == "lab", "sub_field");
    ok = check(ok, d.legs() == 4, "inherit_base_method");
    ok = check(ok, d.speak() == "rex barks", "override_method");
    println(`final=${ok == 1 ? "PASS" : "FAIL"}`);
    return ok == 1 ? 0 : 1;
}
