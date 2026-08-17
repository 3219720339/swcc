// 跨模块基类：Animal（含 speak/legs/hello），供 main 模块的 Dog 继承。
export class Animal {
    name: string;
    constructor(n: string) { this.name = n; }
    speak(): string { return "?"; }
    legs(): int { return 4; }
    hello(): string { return "animal-hello"; }
}
