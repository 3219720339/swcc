import { println } from "std/io";
import { Result } from "std/result";

function add(a: int, b: int): int {
    return a + b;
}

@test function test_add(): int {
    if (add(1, 2) != 3) {
        println("add failed");
        return 1;
    }
    return 0;
}

@test function test_string(): int {
    const text = "hello";
    if (text.length != 5 || text[1] != 'e') {
        println("string failed");
        return 1;
    }
    return 0;
}

@test function test_match(): int {
    const value: Result<int, string> = Result.Ok(7);
    match (value) {
        Ok(v) => {
            return v == 7 ? 0 : 1;
        }
        Err(_) => {
            return 1;
        }
    }
}
