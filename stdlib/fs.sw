// Sw 标准库：文件读写（由运行时实现）。

export extern c function open(path: string, mode: string): int;
export extern c function close(fd: int): int;
export extern c function write(fd: int, text: string): int;
export extern c function read_all(path: string): string;
export extern c function read_line_from(fd: int): string;
export extern c function seek(fd: int, offset: int, origin: int): int;
export extern c function file_size(fd: int): int;
export extern c function exists(path: string): int;
export extern c function path_join(a: string, b: string): string;
