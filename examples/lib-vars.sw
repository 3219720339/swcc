// 跨文件全局变量/常量测试模块：被 probe-vars.sw 引用。
export const VERSION = 3;
export let counter = 10;

export function bump(): int {
    counter += 1;
    return counter;
}
