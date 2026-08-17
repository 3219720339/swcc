// struct 数组作为字段类型（bug 修复：此前编译期报错"暂不支持该字段类型"）。
// 数组是引用类型，字段存 8 字节指针；元素内联存储（struct 字节）。
// 覆盖：字段字面量、递归自引用树、索引读写、push、for-of、map、
// 函数参数、值/引用字段拷贝语义。
import { println } from "std/io";

struct Point {
    x: int;
    y: int;
}

struct Shape {
    label: string;
    points: Point[];
}

struct Node {
    value: int;
    children: Node[];
}

function sum_tree(n: Node): int {
    let total = n.value;
    for (const child of n.children) {
        total = total + sum_tree(child);
    }
    return total;
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

    // 1) struct 数组字段字面量 + 读取
    const line: Shape = { label: "L1", points: [{ x: 1, y: 2 }, { x: 3, y: 4 }] };
    passed = passed & check(line.points.length == 2, "struct array field length");
    passed = passed & check(line.points[0].x == 1 && line.points[0].y == 2, "struct array field read");
    passed = passed & check(line.points[1].x == 3, "struct array field read 2");

    // 2) 索引写入
    let mutable: Shape = { label: "M", points: [{ x: 1, y: 1 }] };
    mutable.points[0].x = 10;
    passed = passed & check(mutable.points[0].x == 10, "struct array element write");

    // 3) push（struct 专用：按元素字节复制）
    mutable.points.push({ x: 3, y: 3 });
    passed = passed & check(mutable.points.length == 2 && mutable.points[1].x == 3 && mutable.points[1].y == 3, "struct array push");

    // 4) 递归自引用树
    const tree: Node = {
        value: 1,
        children: [
            { value: 2, children: [{ value: 4, children: [] }] },
            { value: 3, children: [] },
        ],
    };
    passed = passed & check(sum_tree(tree) == 10, "recursive tree sum");
    passed = passed & check(tree.children[0].children[0].value == 4, "nested tree access");

    // 5) for-of + map
    let total = 0;
    for (const p of line.points) {
        total = total + p.x + p.y;
    }
    passed = passed & check(total == 10, "for-of over struct array");
    const mapped = tree.children.map((c: Node): int => c.value * 10);
    passed = passed & check(mapped[0] == 20 && mapped[1] == 30, "map over struct array");

    // 6) 值/引用字段拷贝语义（文档：值字段拷贝字节、引用字段共享）
    const copy = line;
    copy.label = "B";
    passed = passed & check(line.label == "L1" && copy.label == "B", "value field copied");
    copy.points[0].x = 99;
    passed = passed & check(line.points[0].x == 99, "reference field shared (by design)");

    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
