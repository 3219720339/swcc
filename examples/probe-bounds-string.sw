// 负例：string[] 越界读必须在运行时被拦截（预期退出码 3）。
function main(): int {
    const values: string[] = ["a", "b"];
    return values[7] == "" ? 0 : 1;
}
