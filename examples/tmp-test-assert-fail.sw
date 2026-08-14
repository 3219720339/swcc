import { assert, fail } from "std/test";

@test function passing(): int {
    assert(1 + 1 == 2);
    return 0;
}

@test function failing(): int {
    assert(1 + 1 == 3, "should fail");
    return 0;
}
