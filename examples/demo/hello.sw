import { println } from "std/io";

class Greeter {
    private name: string;
    constructor(name: string) {
        this.name = name;
    }
    greet(): string {
        return `hello ${this.name}`;
    }
}

function sum(values: int[]): int {
    let total = 0;
    for (const value of values) {
        total += value;
    }
    return total;
}

function main(): int {
    const greeter = new Greeter("Sw");
    println(greeter.greet());
    const values = [1, 2, 3, 4];
    println(`sum = ${sum(values)}`);
    return 0;
}
