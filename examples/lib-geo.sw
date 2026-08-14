// 多文件引用测试用模块：定义类型与函数供 probe-multifile.sw 引用。

export struct Point {
    x: int;
    y: float;
}

export class Vector {
    dx: int;
    dy: int;

    constructor(dx: int, dy: int) {
        this.dx = dx;
        this.dy = dy;
    }

    magnitude(): int {
        return this.dx + this.dy;
    }
}

export function make_point(x: int, y: float): Point {
    const p: Point = { x: x, y: y };
    return p;
}
