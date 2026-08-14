import { println } from "std/io";

interface Shape {
    area(): float;
    name(): string;
}

class Circle implements Shape {
    radius: float;
    constructor(r: float) {
        this.radius = r;
    }
    area(): float {
        return 3.14 * this.radius * this.radius;
    }
    name(): string {
        return "circle";
    }
}

class Square implements Shape {
    side: float;
    constructor(s: float) {
        this.side = s;
    }
    area(): float {
        return this.side * this.side;
    }
    name(): string {
        return "square";
    }
}

function describe(shape: Shape): string {
    return `${shape.name()}=${shape.area()}`;
}

function main(): int {
    const circle: Shape = new Circle(2.0);
    const square: Shape = new Square(3.0);
    println(`${describe(circle)} ${describe(square)}`);
    return 0;
}
