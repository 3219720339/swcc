export interface Container<T> {
    get(): T;
    set(value: T): void;
}

export interface Shape {
    area(): float;
}

export class Box<T> implements Container<T> {
    value: T;
    constructor(value: T) { this.value = value; }
    get(): T { return this.value; }
    set(value: T): void { this.value = value; }
}

export class SubBox<T> extends Box<T> {
    extra: int;
    constructor(value: T, extra: int) {
        super(value);
        this.extra = extra;
    }
}

export class Circle implements Shape {
    radius: float;
    constructor(radius: float) { this.radius = radius; }
    area(): float { return 3.14 * this.radius * this.radius; }
}
