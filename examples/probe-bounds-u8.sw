// 负例：u8[] 越界读必须在运行时被拦截（紧凑布局步长 1，预期退出码 3）。
function main(): int {
    const values: u8[] = [1u8, 2u8];
    return values[9] == 0 ? 0 : 1;
}
