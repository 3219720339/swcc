// 数组读写必须在运行时拦截非法下标，禁止落入未定义原生内存。
function main(): int {
    const values: int[] = [7];
    return values[-1];
}
