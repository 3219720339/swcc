function main(): int {
    const captured = 6;
    const worker = () => {
        let i = 0;
        while (i < 8) {
            i++;
            if (i == 3) {
                continue;
            }
            if (i == 7) {
                break;
            }
        }
        return i + captured;
    };
    return worker();
}
