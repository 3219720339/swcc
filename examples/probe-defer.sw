import { println } from "std/io";

function child(): void {
    defer println("child-cleanup");
    throw "child-error";
}

function returns_early(): int {
    defer println("return-cleanup");
    return 7;
}

function main(): int {
    {
        defer println("outer-cleanup");
        defer println("inner-cleanup");
        println("scope-body");
    }

    let i = 0;
    while (i < 3) {
        defer println(`loop-cleanup-${i}`);
        i = i + 1;
        if (i == 2) {
            continue;
        }
        if (i == 3) {
            break;
        }
    }

    if (returns_early() != 7) {
        return 1;
    }
    try {
        defer println("try-cleanup");
        child();
    } catch (e: string) {
        println(e);
    }
    println("defer-pass");
    return 0;
}
