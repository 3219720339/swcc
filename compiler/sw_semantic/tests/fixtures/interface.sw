interface Shape {
    area(): float;
}

class Circle implements Shape {
    radius: float;
    constructor(r: float) {
        this.radius = r;
    }
    area(): float {
        return 3.14 * this.radius * this.radius;
    }
}

function area_of(shape: Shape): float {
    return shape.area();
}

function main(): int {
    const circle: Shape = new Circle(2.0);
    return area_of(circle) as int;
}
