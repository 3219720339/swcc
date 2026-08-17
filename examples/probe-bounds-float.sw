// 负例：float[] 越界读必须在运行时被拦截（预期退出码 3）。
function main(): int {
    const values: float[] = [1.5, 2.5];
    return values[5] >= 0.0 ? 0 : 1;
}
