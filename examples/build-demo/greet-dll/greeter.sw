// DLL 演示：导出函数供外部（C/其他语言）调用，无 main。
export function greet(name: string): string {
    return "Hello, " + name + "!";
}

export function repeat(text: string, times: int): string {
    let out = "";
    for (let i = 0; i < times; i++) {
        out += text;
    }
    return out;
}

export function twice(x: int): int {
    return x * 2;
}
