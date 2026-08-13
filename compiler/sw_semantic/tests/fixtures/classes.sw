class Shape {
    protected name: string;
    constructor(name: string) {
        this.name = name;
    }
    area(): float {
        return 0.0;
    }
}

class Circle extends Shape {
    private radius: float;
    constructor(name: string, radius: float) {
        super(name);
        this.radius = radius;
    }
    override area(): float {
        return 3.14 * this.radius * this.radius;
    }
}

function main(): int {
    const circle = new Circle("c", 1.0);
    const area = circle.area();
    if (area > 0.0) {
        return 42;
    }
    return 0;
}
