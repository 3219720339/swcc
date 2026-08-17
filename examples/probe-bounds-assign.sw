// 负例：赋值目标下标越界（写）必须在运行时被拦截（预期退出码 3）。
function main(): int {
    const values: int[] = [1, 2];
    values[7] = 42;
    return 0;
}
