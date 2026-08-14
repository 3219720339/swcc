// Sw language showcase: declarations, types, control flow, modules and runtime APIs.
import { println } from "std/io";
import { sort_int, sum_int, unique_string } from "std/array";
import { base64_encode, base64_decode } from "std/encoding";
import { gcd, sqrt, pi } from "std/math";
import { json_parse, json_int, json_object_get, json_type_name } from "std/json";
import { time_from_parts, time_format } from "std/time";
import { platform, cwd } from "std/os";
import { path_join, path_basename, path_ext } from "std/fs";

// Top-level globals are shared by every function in this module.
const SHOWCASE_VERSION = 1;
let visits = 0;

type Score = int;

enum Color {
    Red,
    Green,
    Blue,
}

struct Point {
    x: int;
    y: int;
}

struct Pair<T> {
    first: T;
    second: T;
}

interface Shape {
    area(): float;
    label(): string;
}

class Circle implements Shape {
    radius: float;

    constructor(radius: float) {
        this.radius = radius;
    }

    area(): float {
        return pi() * this.radius * this.radius;
    }

    label(): string {
        return "circle";
    }
}

class InheritedCircle extends Circle {
    constructor(radius: float) {
        super(radius);
    }
}

class Counter {
    value: int;

    constructor(value: int) {
        this.value = value;
    }

    increment(): int {
        this.value += 1;
        return this.value;
    }
}

class NamedCounter extends Counter {
    name: string;

    constructor(name: string, value: int) {
        super(value);
        this.name = name;
    }
}

// An extern declaration is checked with the C ABI. It is intentionally unused.
extern c function showcase_native(value: int): int;

function identity<T>(value: T): T {
    return value;
}

function add(a: int, b: int = 1): int {
    return a + b;
}

function point_sum(point: Point): int {
    return point.x + point.y;
}

function guarded(flag: bool): int {
    if (flag) {
        throw "showcase error";
    }
    return 42;
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
    println("showcase: start");
    visits += 1;
    let passed = 1;

    // Numeric, boolean, char, explicit casts, operators and assignments.
    const integer: i32 = 7;
    const unsigned: u8 = 3u8;
    const character: char = 'S';
    const floating: float = integer as float;
    let arithmetic: int = (integer as int) * 2 + (unsigned as int);
    arithmetic++;
    arithmetic -= 1;
    passed = passed & check(arithmetic == 17 && character == 'S', "primitive types and operators");
    passed = passed & check(floating == 7.0 && (8 ** 2) == 64, "float and power");

    // Struct values, nested arrays, indexing, C-style for, for/of and equality.
    const point: Point = { x: 2, y: 3 };
    const pair: Pair<int> = { first: 4, second: 5 };
    const numbers = [3, 1, 2];
    sort_int(numbers);
    let total = 0;
    for (const number of numbers) {
        total += number;
    }
    let index = 0;
    while (index < numbers.length) {
        index++;
    }
    for (let step = 0; step < 2; step++) {
        total += step;
    }
    total += point_sum(point);
    passed = passed & check(total == 12 && index == 3 && pair.first + pair.second == 9, "struct, array and control flow");
    passed = passed & check(sum_int(numbers) == 6 && numbers[1] == 2, "typed array standard library");
    let selected = 0;
    switch (pair.first) {
        case 4:
            selected = add(4);
            break;
        default:
            selected = -1;
    }
    passed = passed & check(selected == 5, "switch, break and default argument");

    // Classes, inheritance, interface dispatch and nullable chaining.
    const shape: Shape = new InheritedCircle(2.0);
    const named_counter = new NamedCounter("visits", 2);
    const maybe_shape: Shape? = shape;
    const missing: Circle? = null;
    const area_text = "area=" + sqrt(4.0);
    const missing_label = missing?.label() ?? "missing";
    passed = passed & check(shape.label() == "circle" && area_text.length > 5, "class and interface");
    passed = passed & check(named_counter.increment() == 3 && named_counter.name == "visits", "class inheritance");
    passed = passed & check(missing_label == "missing", "nullable and optional chain");

    // Lambdas capture local values; generic functions are specialized at the call site.
    const offset = 10;
    const closure = (value: int) => value + offset;
    const generic_value: Score = identity(closure(5));
    passed = passed & check(generic_value == 15 && add(4, 1) == 5, "closure and generic function");

    // Strings, templates, encoding and JSON pointers.
    const encoded = base64_encode("Sw");
    const words = unique_string(["sw", "sw", "lang"]);
    const json = json_parse("{\"answer\":42,\"ok\":true}");
    const answer = json_int(json_object_get(json, "answer"));
    const message = "answer " + answer + " " + sqrt(4.0);
    passed = passed & check(`v${SHOWCASE_VERSION} ${base64_decode(encoded)}` == "v1 Sw", "strings, templates and encoding");
    passed = passed & check(words.length == 2 && message == "answer 42 2", "string conversion and JSON");
    passed = passed & check(json_type_name(json) == "object", "JSON type inspection");

    // Time, OS and path APIs are deterministic enough for structural checks.
    const timestamp = time_from_parts(2026, 1, 2, 3, 4, 5);
    const stamp = time_format(timestamp, "%Y-%m-%d");
    const joined = path_join("examples", "showcase.sw");
    passed = passed & check(stamp == "2026-01-02" && platform().length > 0, "time and platform");
    passed = passed & check(cwd().length > 0 && path_basename(joined) == "showcase.sw", "OS and paths");
    passed = passed & check(path_ext(joined) == ".sw", "path extension");

    // try/catch/finally and throw.
    let caught = 0;
    try {
        guarded(true);
    } catch (error: string) {
        caught = error == "showcase error" ? 1 : 0;
    } finally {
        println("finally: scope completed");
    }
    passed = passed & check(caught == 1 && visits == 1, "exceptions and global state");

    println(`showcase=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
