// Sw 标准库：文件读写（由运行时实现）。

export extern c function open(path: string, mode: string): int;
export extern c function close(fd: int): int;
export extern c function write(fd: int, text: string): int;
export extern c function read_all(path: string): string;
