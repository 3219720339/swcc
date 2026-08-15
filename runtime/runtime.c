// Sw 运行时（v0.1 最小实现）
// 无系统头文件依赖：仅显式声明用到的 CRT 函数，可同时用 MSVC/MinGW 工具链编译。

#pragma clang diagnostic ignored "-Wincompatible-library-redeclaration"
#pragma clang diagnostic ignored "-Wbuiltin-requires-header"

typedef long long int64_t;
typedef unsigned long long uint64_t;
typedef unsigned long long uintptr_t;
typedef unsigned int uint32_t;
#define NULL ((void*)0)

#if defined(_WIN32)
typedef unsigned long long sw_size;
#else
typedef unsigned long sw_size;
#endif

extern void* malloc(sw_size size);
extern void free(void* ptr);
extern void* calloc(sw_size count, sw_size size);
extern void* realloc(void* ptr, sw_size size);
extern void* memcpy(void* dest, const void* src, sw_size count);
extern void* memmove(void* dest, const void* src, sw_size count);
extern void* memset(void* dest, int value, sw_size count);
extern int memcmp(const void* a, const void* b, sw_size count);
extern int snprintf(char* buffer, sw_size size, const char* format, ...);
extern uint64_t fwrite(const void* data, sw_size size, sw_size count, void* stream);
extern int fputc(int character, void* stream);
extern int fgetc(void* stream);
extern int fflush(void* stream);
extern uint64_t strlen(const char* text);
extern void exit(int code);
#if defined(_WIN32)
extern void* __acrt_iob_func(unsigned int index);
#define stdout __acrt_iob_func(1)
#define stdin __acrt_iob_func(0)
#define stderr __acrt_iob_func(2)
#elif defined(__APPLE__)
// macOS 上 stdin/stdout 是宏，真实符号为 ___stdinp / ___stdoutp。
extern void* __stdinp;
extern void* __stdoutp;
extern void* __stderrp;
#define stdin __stdinp
#define stdout __stdoutp
#define stderr __stderrp
#else
extern void* stdin;
extern void* stdout;
extern void* stderr;
#endif

// 汇编实现的 setjmp/longjmp（runtime_x64.S / runtime_aarch64.s）
extern int sw_setjmp(void* buf);
extern void sw_longjmp(void* buf, int value);

typedef struct {
    char* data;
    int64_t len;
} sw_string;

typedef struct {
    int64_t len;
    int64_t cap;
    void* data;
} sw_array;

// ---------------------------------------------------------------------------
// 内存管理：保守式标记-清除 GC。
// 扫描根：原生栈、寄存器溢出区、程序数据段；字符串/数组/对象/闭包全部经
// sw_gc_alloc 分配，达到阈值自动回收不可达块。误判只导致少回收（安全）。
// ---------------------------------------------------------------------------

#define SW_GC_HEADER_SIZE 32
#define SW_GC_INIT_THRESHOLD (4u << 20)
// 阈值翻倍上限：过高会导致高分配场景 GC 触发过少、死对象堆积
// （此前 128MB 时 30 万次异常/字符串残留 50MB+）；32MB 平衡频率与峰值。
#define SW_GC_MAX_THRESHOLD (32u << 20)
#define SW_GC_MAX_DATA_RANGES 32

typedef struct sw_gc_block {
    struct sw_gc_block* next;
    uint64_t size;
    uint64_t marked;
    uint64_t _pad;
} sw_gc_block;

static sw_gc_block* sw_gc_blocks = NULL;
static uint64_t sw_gc_allocated_since_collect = 0;
static uint64_t sw_gc_threshold = SW_GC_INIT_THRESHOLD;
static int sw_gc_in_collect = 0;
static int sw_gc_initialized = 0;
static sw_gc_block** sw_gc_sorted = NULL;
static uint64_t sw_gc_sorted_count = 0;

// GC 暂停计数：C 函数在"纯内部计算 + 大量临时 GC 对象"期间暂停回收，
// 避免保守式栈扫描对临时对象的误回收（rx 引擎节点树/码点数组等）。
// 暂停期间 sw_gc_alloc 照常分配（不触发 collect），恢复后由后续 GC 回收。
static int sw_gc_disabled = 0;

static void sw_gc_disable(void) {
    sw_gc_disabled++;
}

static void sw_gc_enable(void) {
    if (sw_gc_disabled > 0) {
        sw_gc_disabled--;
    }
}

static int sw_gc_data_range_count = 0;
static uintptr_t sw_gc_data_ranges[SW_GC_MAX_DATA_RANGES][2];

static void sw_gc_add_data_range(uintptr_t start, uintptr_t end) {
    if (sw_gc_data_range_count >= SW_GC_MAX_DATA_RANGES || start >= end) {
        return;
    }
    sw_gc_data_ranges[sw_gc_data_range_count][0] = start;
    sw_gc_data_ranges[sw_gc_data_range_count][1] = end;
    sw_gc_data_range_count++;
}

#if defined(_WIN32) && defined(__x86_64__)
static uintptr_t sw_read_gs(uint64_t offset) {
    uintptr_t value;
    __asm__ __volatile__("movq %%gs:(%1), %0" : "=r"(value) : "r"((uintptr_t)offset));
    return value;
}

static void sw_gc_init_platform(void) {
    uintptr_t peb = sw_read_gs(0x60);
    uintptr_t image_base = *(uintptr_t*)(peb + 0x10);
    uint32_t e_lfanew = *(uint32_t*)(image_base + 0x3C);

    // PE 头：e_lfanew 指向 "PE\0\0"；+4 是 COFF 头（20 字节：
    // +0 Machine, +2 NumberOfSections, +16 SizeOfOptionalHeader, +18 Characteristics）。
    uintptr_t pe_header = image_base + e_lfanew;
    uint32_t number_of_sections = (uint32_t)(*(unsigned short*)(pe_header + 4 + 2));
    uint32_t size_of_optional = (uint32_t)(*(unsigned short*)(pe_header + 4 + 16));
    // OptionalHeader 起始 = pe_header + 4 + 20；节表紧随 OptionalHeader 之后。
    uintptr_t optional = pe_header + 4 + 20;
    // 只注册可写数据节（.data/.rdata/.bss 等），排除 .text 代码段——
    // 指令字节会被保守式扫描误判为指向堆块的指针，导致全部块被标记、
    // GC 永不回收（字符串/异常对象泄漏）。
    uintptr_t section_table = optional + size_of_optional;
    for (uint32_t index = 0; index < number_of_sections; index++) {
        uintptr_t section = section_table + (uintptr_t)index * 40;
        uint32_t characteristics = *(uint32_t*)(section + 36);
        uint32_t virtual_size = *(uint32_t*)(section + 8);
        uint32_t virtual_address = *(uint32_t*)(section + 12);
        // IMAGE_SCN_MEM_WRITE (0x80000000) 或非 IMAGE_SCN_CNT_CODE (0x20)。
        if ((characteristics & 0x80000000u) != 0 || (characteristics & 0x20u) == 0) {
            if (virtual_size == 0) {
                virtual_size = *(uint32_t*)(section + 16);  // SizeOfRawData 兜底
            }
            sw_gc_add_data_range(
                image_base + virtual_address,
                image_base + virtual_address + virtual_size
            );

        }
    }
}

static uintptr_t sw_stack_top(void) {
    return sw_read_gs(0x08);
}
#elif defined(__linux__)
struct sw_phdr {
    uint32_t type;
    uint32_t flags;
    uint64_t offset;
    uint64_t vaddr;
    uint64_t paddr;
    uint64_t filesz;
    uint64_t memsz;
    uint64_t align;
};

struct sw_dl_info {
    uintptr_t dlpi_addr;
    const char* dlpi_name;
    const void* dlpi_phdr;
    unsigned short dlpi_phnum;
    unsigned short _pad;
    uint64_t dlpi_adds;
    uint64_t dlpi_subs;
    void* dlpi_tls_modid;
    void* dlpi_tls_data;
};

extern int dl_iterate_phdr(int (*callback)(struct sw_dl_info*, uint64_t, void*), void* data);

static int sw_gc_dl_callback(struct sw_dl_info* info, uint64_t size, void* data) {
    (void)size;
    (void)data;
    const struct sw_phdr* phdr = (const struct sw_phdr*)info->dlpi_phdr;
    for (unsigned short index = 0; index < info->dlpi_phnum; index++) {
        if (phdr[index].type == 1 && phdr[index].memsz > 0) {
            sw_gc_add_data_range(
                info->dlpi_addr + phdr[index].vaddr,
                info->dlpi_addr + phdr[index].vaddr + phdr[index].memsz
            );
        }
    }
    return 0;
}

static void sw_gc_init_platform(void) {
    dl_iterate_phdr(sw_gc_dl_callback, NULL);
}
#elif defined(__APPLE__)
extern unsigned int _dyld_image_count(void);
extern const void* _dyld_get_image_header(unsigned int index);
extern long _dyld_get_image_vmaddr_slide(unsigned int index);

static void sw_gc_init_platform(void) {
    unsigned int count = _dyld_image_count();
    for (unsigned int index = 0; index < count; index++) {
        const unsigned char* header =
            (const unsigned char*)_dyld_get_image_header(index);
        long slide = _dyld_get_image_vmaddr_slide(index);
        if (header == NULL || *(uint32_t*)header != 0xFEEDFACF) {
            continue;
        }
        uint32_t ncmds = *(uint32_t*)(header + 16);
        const unsigned char* cmd = header + 32;
        for (uint32_t i = 0; i < ncmds; i++) {
            uint32_t cmd_type = *(uint32_t*)cmd;
            uint32_t cmd_size = *(uint32_t*)(cmd + 4);
            if (cmd_type == 0x19 && cmd_size >= 72) {
                uint64_t vmaddr = *(uint64_t*)(cmd + 24);
                uint64_t vmsize = *(uint64_t*)(cmd + 32);
                uint64_t filesize = *(uint64_t*)(cmd + 48);
                if (filesize > 0) {
                    uint64_t size = vmsize ? vmsize : filesize;
                    sw_gc_add_data_range(
                        (uintptr_t)(slide + vmaddr),
                        (uintptr_t)(slide + vmaddr + size)
                    );
                }
            }
            cmd += cmd_size;
        }
    }
}
#else
static void sw_gc_init_platform(void) {}
#endif

#if !defined(_WIN32)
static uintptr_t sw_stack_top(void) {
#if defined(__APPLE__)
    // macOS 主线程上 pthread_getattr_np 可能失败；用专用 API 取栈顶（高地址）。
    extern void* pthread_self(void);
    extern void* pthread_get_stackaddr_np(void* thread);
    return (uintptr_t)pthread_get_stackaddr_np(pthread_self());
#else
    char attr[512];
    uintptr_t top = 0;
    extern void* pthread_self(void);
    extern int pthread_getattr_np(void* thread, void* attr);
    extern int pthread_attr_getstack(const void* attr, void** stackaddr, uint64_t* stacksize);
    extern int pthread_attr_destroy(void* attr);
    if (pthread_getattr_np(pthread_self(), attr) == 0) {
        void* addr = NULL;
        uint64_t size = 0;
        if (pthread_attr_getstack(attr, &addr, &size) == 0) {
            top = (uintptr_t)addr + size;
        }
        pthread_attr_destroy(attr);
    }
    return top;
#endif
}
#endif

static uintptr_t sw_gc_align8(uintptr_t value) {
    return (value + 7u) & ~(uintptr_t)7u;
}

extern void qsort(void* base, uint64_t count, uint64_t size, int (*compare)(const void*, const void*));

static int sw_gc_compare_blocks(const void* a, const void* b) {
    const sw_gc_block* block_a = *(const sw_gc_block* const*)a;
    const sw_gc_block* block_b = *(const sw_gc_block* const*)b;
    uintptr_t payload_a = (uintptr_t)((const char*)block_a + SW_GC_HEADER_SIZE);
    uintptr_t payload_b = (uintptr_t)((const char*)block_b + SW_GC_HEADER_SIZE);
    return payload_a < payload_b ? -1 : (payload_a > payload_b ? 1 : 0);
}

static void sw_gc_mark_word(uintptr_t word) {
    if (word == 0 || sw_gc_sorted_count == 0) {
        return;
    }
    // 二分查找最后一个 payload <= word 的块。
    uint64_t lo = 0;
    uint64_t hi = sw_gc_sorted_count;
    while (lo < hi) {
        uint64_t mid = (lo + hi) / 2;
        uintptr_t payload = (uintptr_t)((char*)sw_gc_sorted[mid] + SW_GC_HEADER_SIZE);
        if (payload <= word) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if (lo > 0) {
        sw_gc_block* block = sw_gc_sorted[lo - 1];
        uintptr_t payload = (uintptr_t)((char*)block + SW_GC_HEADER_SIZE);
        if (word < payload + block->size) {
            block->marked = 1;
        }
    }
}

static void sw_gc_scan_words(uintptr_t start, uintptr_t end) {
    start = sw_gc_align8(start);
    for (uintptr_t word = start; word + 8 <= end; word += 8) {
        sw_gc_mark_word(*(uintptr_t*)word);
    }
}

/// 扫描堆块内容：指向本块内部的指针（如 sw_string.data 指向块内联数据）
/// 是自引用，不能用于标记存活；只把指向其它 GC 块的指针当外部引用。
static void sw_gc_scan_block(uintptr_t payload, uintptr_t end) {
    payload = sw_gc_align8(payload);
    for (uintptr_t word = payload; word + 8 <= end; word += 8) {
        uintptr_t value = *(uintptr_t*)word;
        if (value >= payload && value < end) {
            continue;  // 指向本块内部：自引用，跳过
        }
        sw_gc_mark_word(value);
    }
}

static void sw_gc_scan_stack(void) {
    char dummy;
    uintptr_t sp = (uintptr_t)&dummy;
    uintptr_t top = sw_stack_top();
    if (top > sp) {
        sw_gc_scan_words(sp, top);
    }
    // 寄存器溢出区：把被调用者保存寄存器挤到本帧缓冲区再扫描。
    unsigned char regs[0x140];
    sw_setjmp(regs);
    sw_gc_scan_words((uintptr_t)regs, (uintptr_t)regs + sizeof(regs));
}

static void sw_gc_collect(void) {
    sw_gc_in_collect = 1;
    uint64_t count = 0;
    for (sw_gc_block* block = sw_gc_blocks; block != NULL; block = block->next) {
        block->marked = 0;
        count++;
    }
    sw_gc_sorted = (sw_gc_block**)malloc((sw_size)(count * sizeof(sw_gc_block*)));
    if (sw_gc_sorted != NULL && count > 0) {
        uint64_t index = 0;
        for (sw_gc_block* block = sw_gc_blocks; block != NULL; block = block->next) {
            sw_gc_sorted[index++] = block;
        }
        qsort(sw_gc_sorted, count, sizeof(sw_gc_block*), sw_gc_compare_blocks);
    }
    sw_gc_sorted_count = count;

    for (int index = 0; index < sw_gc_data_range_count; index++) {
        sw_gc_scan_words(sw_gc_data_ranges[index][0], sw_gc_data_ranges[index][1]);
    }
    sw_gc_scan_stack();
    // 堆内引用：类字段/数组元素可能指向其它 GC 块（保守式，多扫无害）。
    // 指向本块内部的指针（字符串 data 内联）不标记（自引用）。
    for (sw_gc_block* block = sw_gc_blocks; block != NULL; block = block->next) {
        uintptr_t payload = (uintptr_t)((char*)block + SW_GC_HEADER_SIZE);
        sw_gc_scan_block(payload, payload + block->size);
    }
    if (sw_gc_sorted != NULL) {
        free(sw_gc_sorted);
        sw_gc_sorted = NULL;
    }
    sw_gc_sorted_count = 0;

    sw_gc_block** link = &sw_gc_blocks;
    while (*link != NULL) {
        sw_gc_block* block = *link;
        if (block->marked) {
            block->marked = 0;
            link = &block->next;
        } else {
            *link = block->next;
            free(block);
        }
    }

    sw_gc_allocated_since_collect = 0;
    sw_gc_in_collect = 0;
}

static void* sw_gc_alloc(uint64_t size) {
    if (!sw_gc_initialized) {
        sw_gc_init_platform();
        sw_gc_initialized = 1;
    }
    if (size < 8) {
        size = 8;
    }
    if (!sw_gc_in_collect && sw_gc_disabled == 0 &&
        sw_gc_allocated_since_collect >= sw_gc_threshold) {
        sw_gc_collect();
        if (sw_gc_threshold < SW_GC_MAX_THRESHOLD) {
            sw_gc_threshold *= 2;
        }
    }
    sw_gc_block* block = (sw_gc_block*)malloc(SW_GC_HEADER_SIZE + (sw_size)size);
    if (block == NULL) {
        return NULL;
    }
    block->next = sw_gc_blocks;
    block->size = size;
    block->marked = 0;
    block->_pad = 0;
    sw_gc_blocks = block;
    sw_gc_allocated_since_collect += size + SW_GC_HEADER_SIZE;
    return (char*)block + SW_GC_HEADER_SIZE;
}

// 字符串：结构体与数据区合并为一个 GC 块。

sw_string* sw_string_from_literal(const char* data, int64_t len) {
    if (len < 0) {
        len = 0;
    }
    sw_string* string =
        (sw_string*)sw_gc_alloc(sizeof(sw_string) + (uint64_t)len + 1);
    char* copy = (char*)(string + 1);
    if (len > 0) {
        memcpy(copy, data, (uint64_t)len);
    }
    copy[len] = 0;
    string->data = copy;
    string->len = len;
    return string;
}

sw_string* sw_string_concat(sw_string* a, sw_string* b) {
    int64_t len = a->len + b->len;
    sw_string* string =
        (sw_string*)sw_gc_alloc(sizeof(sw_string) + (uint64_t)len + 1);
    char* data = (char*)(string + 1);
    if (a->len > 0) {
        memcpy(data, a->data, (uint64_t)a->len);
    }
    if (b->len > 0) {
        memcpy(data + a->len, b->data, (uint64_t)b->len);
    }
    data[len] = 0;
    string->data = data;
    string->len = len;
    return string;
}

sw_string* sw_int_to_string(int64_t value) {
    char buffer[64];
    int len = snprintf(buffer, sizeof(buffer), "%lld", value);
    return sw_string_from_literal(buffer, len);
}

sw_string* sw_uint_to_string(uint64_t value) {
    char buffer[64];
    int len = snprintf(buffer, sizeof(buffer), "%llu", value);
    return sw_string_from_literal(buffer, len);
}

sw_string* sw_float_to_string(double value) {
    char buffer[64];
    int len = snprintf(buffer, sizeof(buffer), "%g", value);
    return sw_string_from_literal(buffer, len);
}

sw_string* sw_char_to_string(int64_t value) {
    char buffer[8];
    int len = 0;
    if (value < 0x80) {
        buffer[len++] = (char)value;
    } else if (value < 0x800) {
        buffer[len++] = (char)(0xC0 | (value >> 6));
        buffer[len++] = (char)(0x80 | (value & 0x3F));
    } else if (value < 0x10000) {
        buffer[len++] = (char)(0xE0 | (value >> 12));
        buffer[len++] = (char)(0x80 | ((value >> 6) & 0x3F));
        buffer[len++] = (char)(0x80 | (value & 0x3F));
    } else {
        buffer[len++] = (char)(0xF0 | (value >> 18));
        buffer[len++] = (char)(0x80 | ((value >> 12) & 0x3F));
        buffer[len++] = (char)(0x80 | ((value >> 6) & 0x3F));
        buffer[len++] = (char)(0x80 | (value & 0x3F));
    }
    return sw_string_from_literal(buffer, len);
}

sw_string* sw_bool_to_string(int64_t value) {
    return sw_string_from_literal(value ? "true" : "false", value ? 4 : 5);
}

// 可变参数运行时类型标签（与编译器 SW_TAG_* 保持一致）。
#define SW_TAG_INT 0
#define SW_TAG_FLOAT 1
#define SW_TAG_STR 2
#define SW_TAG_BOOL 3
#define SW_TAG_CHAR 4

void sw_print_string(sw_string* string) {
    if (string->len > 0) {
        fwrite(string->data, 1, (uint64_t)string->len, stdout);
    }
}

// 按 varargs 打包数组输出任意类型（每元素两槽：tag, value）。
void sw_print_any(sw_array* args) {
    if (args != NULL && args->len > 0) {
        int64_t* data = (int64_t*)args->data;
        // args->len 是参数对数，每对占两槽（tag, value）。
        for (int64_t i = 0; i + 1 < args->len * 2; i += 2) {
            if (i > 0) {
                fputc(' ', stdout);
            }
            int64_t tag = data[i];
            int64_t value = data[i + 1];
            switch (tag) {
                case SW_TAG_INT: {
                    sw_string* text = sw_int_to_string(value);
                    sw_print_string(text);
                    break;
                }
                case SW_TAG_FLOAT: {
                    double d;
                    memcpy(&d, &value, 8);
                    sw_string* text = sw_float_to_string(d);
                    sw_print_string(text);
                    break;
                }
                case SW_TAG_STR: {
                    sw_string* text = (sw_string*)value;
                    if (text != NULL) {
                        sw_print_string(text);
                    }
                    break;
                }
                case SW_TAG_BOOL: {
                    sw_string* text = sw_bool_to_string(value);
                    sw_print_string(text);
                    break;
                }
                case SW_TAG_CHAR: {
                    sw_string* text = sw_char_to_string(value);
                    sw_print_string(text);
                    break;
                }
                default: {
                    sw_string* text = sw_int_to_string(value);
                    sw_print_string(text);
                    break;
                }
            }
        }
    }
}

void println(sw_array* args) {
    sw_print_any(args);
    fputc('\n', stdout);
}

void print(sw_array* args) {
    sw_print_any(args);
}

// 冲刷 stdout（进度条/倒计时提示等场景 print 后立即输出需要）。
void sw_flush(void) {
    fflush(stdout);
}

// 等待按键后继续（防止控制台窗口运行完立刻关闭）。
void sw_pause(void) {
    const char* msg = "请按任意键继续...";
    fwrite(msg, 1, strlen(msg), stdout);
    fflush(stdout);
#if defined(_WIN32)
    extern int _getch(void);
    _getch();
#else
    fgetc(stdin);
#endif
}

// ---------------------------------------------------------------------------
// 交互控制台（std/console）：ANSI 颜色/清屏/光标、读键、终端尺寸。
// Windows 需要先启用 VT 转义序列处理（Win10 1511+；重定向到管道/文件时
// GetConsoleMode 失败自动跳过，不影响 CI 输出捕获）。
// ---------------------------------------------------------------------------

#if defined(_WIN32)
static void sw_enable_vt(void) {
    extern void* GetStdHandle(int nStdHandle);
    extern int GetConsoleMode(void* handle, unsigned long* mode);
    extern int SetConsoleMode(void* handle, unsigned long mode);
    void* handle = GetStdHandle(-11); // STD_OUTPUT_HANDLE
    unsigned long mode = 0;
    if (handle != NULL && GetConsoleMode(handle, &mode) != 0) {
        SetConsoleMode(handle, mode | 0x0004u); // ENABLE_VIRTUAL_TERMINAL_PROCESSING
    }
}
#endif

static void sw_console_write(const char* text) {
    fwrite(text, 1, strlen(text), stdout);
    fflush(stdout);
}

// console_color(fg, bg)：0-7 基本色（0 黑 1 红 2 绿 3 黄 4 蓝 5 品红 6 青 7 白）；
// -1 表示该位不变；两个都是 -1 时重置为默认。输出 ANSI SGR 序列。
void sw_console_color(int64_t fg, int64_t bg) {
    char buffer[40];
    int used = 0;
    if (fg >= 0 && fg < 8) {
        used += snprintf(buffer + used, (sw_size)(sizeof(buffer) - (sw_size)used),
                         "\x1b[%dm", (int)(30 + fg));
    }
    if (bg >= 0 && bg < 8) {
        used += snprintf(buffer + used, (sw_size)(sizeof(buffer) - (sw_size)used),
                         "\x1b[%dm", (int)(40 + bg));
    }
    if (used == 0) {
        sw_console_write("\x1b[0m");
        return;
    }
    sw_console_write(buffer);
}

void sw_console_reset(void) {
    sw_console_write("\x1b[0m");
}

void sw_console_clear(void) {
    sw_console_write("\x1b[2J\x1b[H");
}

// console_gotoxy(x, y)：1 基坐标定位光标（ANSI \x1b[row;colH）。
void sw_console_gotoxy(int64_t x, int64_t y) {
    char buffer[32];
    int used = snprintf(buffer, sizeof(buffer), "\x1b[%d;%dH", (int)y, (int)x);
    sw_console_write(buffer);
    (void)used;
}

void sw_console_hide_cursor(void) {
    sw_console_write("\x1b[?25l");
}

void sw_console_show_cursor(void) {
    sw_console_write("\x1b[?25h");
}

// 清空当前行并回到行首（进度条重绘用）。
void sw_console_clear_line(void) {
    sw_console_write("\x1b[2K\r");
}

// 设置终端窗口标题（Windows SetConsoleTitleA / POSIX ANSI OSC 序列）。
void sw_console_title(sw_string* text) {
#if defined(_WIN32)
    extern int SetConsoleTitleA(const char* title);
    if (text != NULL) {
        char* copy = (char*)sw_gc_alloc((uint64_t)text->len + 1);
        memcpy(copy, text->data, (uint64_t)text->len);
        copy[text->len] = 0;
        SetConsoleTitleA(copy);
    }
#else
    if (text != NULL) {
        fwrite("\x1b]0;", 1, 4, stdout);
        fwrite(text->data, 1, (uint64_t)text->len, stdout);
        fwrite("\x07", 1, 1, stdout);
        fflush(stdout);
    }
#endif
}

// getch()：读一个按键不回车。Windows _getch；POSIX termios 原始模式单字节读。
// 返回键码（0-255）；失败返回 -1。方向键等扩展键在 Windows 上是双字节序列，
// 本实现只返回首字节（完整扩展键留待后续）。
int64_t sw_getch(void) {
#if defined(_WIN32)
    extern int _getch(void);
    return (int64_t)_getch();
#else
    // POSIX termios 布局（仅操作 c_lflag；c_cc 偏移按平台排，tcsetattr 需要）。
    typedef struct {
#if defined(__APPLE__)
        unsigned long c_iflag, c_oflag, c_cflag, c_lflag;
        unsigned char c_cc[20];
#else
        unsigned int c_iflag, c_oflag, c_cflag, c_lflag;
        unsigned char c_line;
        unsigned char c_cc[32];
#endif
    } sw_termios_ctx;
    extern int tcgetattr(int fd, void* termios);
    extern int tcsetattr(int fd, int actions, const void* termios);
    extern long read(int fd, void* buffer, unsigned long count);
    sw_termios_ctx original;
    sw_termios_ctx raw;
    if (tcgetattr(0, &original) != 0) {
        return -1;
    }
    raw = original;
    raw.c_lflag &= ~(0x2u | 0x8u); // ICANON | ECHO
    if (tcsetattr(0, 0 /*TCSANOW*/, &raw) != 0) {
        return -1;
    }
    unsigned char byte = 0;
    long got = read(0, &byte, 1);
    tcsetattr(0, 0 /*TCSANOW*/, &original);
    return got == 1 ? (int64_t)byte : -1;
#endif
}

// 终端宽/高（字符数）。非控制台（重定向/CI 管道）返回 0。
int64_t sw_console_width(void) {
#if defined(_WIN32)
    extern void* GetStdHandle(int nStdHandle);
    extern int GetConsoleScreenBufferInfo(void* handle, void* info);
    struct {
        short size_x, size_y;      // COORD dwSize
        short cursor_x, cursor_y;  // COORD dwCursorPosition
        short attributes;          // WORD wAttributes
        short win_left, win_top, win_right, win_bottom; // SMALL_RECT srWindow
        short max_x, max_y;        // COORD dwMaximumWindowSize
    } info;
    void* handle = GetStdHandle(-11);
    if (handle != NULL && GetConsoleScreenBufferInfo(handle, &info) != 0) {
        return (int64_t)info.size_x;
    }
    return 0;
#else
    extern int ioctl(int fd, unsigned long request, void* arg);
    struct {
        unsigned short rows, cols, xpixel, ypixel;
    } winsize;
    unsigned long request;
#if defined(__APPLE__)
    request = 0x40087468; // TIOCGWINSZ (macOS)
#else
    request = 0x5413; // TIOCGWINSZ (Linux)
#endif
    if (ioctl(0, request, &winsize) == 0) {
        return (int64_t)winsize.cols;
    }
    return 0;
#endif
}

int64_t sw_console_height(void) {
#if defined(_WIN32)
    extern void* GetStdHandle(int nStdHandle);
    extern int GetConsoleScreenBufferInfo(void* handle, void* info);
    struct {
        short size_x, size_y;
        short cursor_x, cursor_y;
        short attributes;
        short win_left, win_top, win_right, win_bottom;
        short max_x, max_y;
    } info;
    void* handle = GetStdHandle(-11);
    if (handle != NULL && GetConsoleScreenBufferInfo(handle, &info) != 0) {
        return (int64_t)info.size_y;
    }
    return 0;
#else
    extern int ioctl(int fd, unsigned long request, void* arg);
    struct {
        unsigned short rows, cols, xpixel, ypixel;
    } winsize;
    unsigned long request;
#if defined(__APPLE__)
    request = 0x40087468; // TIOCGWINSZ (macOS)
#else
    request = 0x5413; // TIOCGWINSZ (Linux)
#endif
    if (ioctl(0, request, &winsize) == 0) {
        return (int64_t)winsize.rows;
    }
    return 0;
#endif
}

// 测试 runner 专用：按单个字符串输出一行（@test 的 [ok]/[FAIL] 打印）。
void sw_test_println(sw_string* string) {
    sw_print_string(string);
    fputc('\n', stdout);
}

extern char* fgets(char* buffer, int size, void* stream);

sw_string* read_line(void) {
    char buffer[4096];
    if (fgets(buffer, sizeof(buffer), stdin) == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t len = 0;
    while (len < (int64_t)sizeof(buffer) && buffer[len] != 0 && buffer[len] != '\n') {
        len++;
    }
    if (len > 0 && buffer[len - 1] == '\r') {
        len--;
    }
    return sw_string_from_literal(buffer, len);
}

// ---------------------------------------------------------------------------
// math：abs/floor/ceil/sqrt 等（sqrt 等走 libm，三平台均有）
// ---------------------------------------------------------------------------

int64_t sw_abs(int64_t value) {
    return value < 0 ? -value : value;
}

int64_t min(int64_t a, int64_t b) {
    return a < b ? a : b;
}

int64_t max(int64_t a, int64_t b) {
    return a > b ? a : b;
}

// ---------------------------------------------------------------------------
// fs：文件描述符表（最多 64 个），读写走 libc。
// ---------------------------------------------------------------------------

#define SW_MAX_FILES 64

typedef void sw_file_handle;

static sw_file_handle* sw_files[SW_MAX_FILES] = {0};

extern sw_file_handle* fopen(const char* path, const char* mode);
extern int fclose(sw_file_handle* file);
extern uint64_t fread(void* data, uint64_t size, uint64_t count, sw_file_handle* file);
extern int fseek(sw_file_handle* file, long offset, int origin);
extern long ftell(sw_file_handle* file);
extern void rewind(sw_file_handle* file);
extern char* fgets(char* buffer, int size, void* stream);

int64_t sw_open(sw_string* path, sw_string* mode) {
    for (int64_t index = 0; index < SW_MAX_FILES; index++) {
        if (sw_files[index] == NULL) {
            sw_file_handle* file = fopen(path->data, mode->data);
            if (file == NULL) {
                return -1;
            }
            sw_files[index] = file;
            return index;
        }
    }
    return -1;
}

int64_t sw_close(int64_t fd) {
    if (fd < 0 || fd >= SW_MAX_FILES || sw_files[fd] == NULL) {
        return -1;
    }
    int result = fclose(sw_files[fd]);
    sw_files[fd] = NULL;
    return result == 0 ? 0 : -1;
}

int64_t sw_write(int64_t fd, sw_string* text) {
    if (fd < 0 || fd >= SW_MAX_FILES || sw_files[fd] == NULL) {
        return -1;
    }
    return (int64_t)fwrite(text->data, 1, (uint64_t)text->len, sw_files[fd]);
}

sw_string* read_line_from(int64_t fd) {
    if (fd < 0 || fd >= SW_MAX_FILES || sw_files[fd] == NULL) {
        return sw_string_from_literal("", 0);
    }
    char buffer[4096];
    if (fgets(buffer, 4096, sw_files[fd]) == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t len = 0;
    while (len < 4096 && buffer[len] != 0 && buffer[len] != '\n') {
        len++;
    }
    if (len > 0 && buffer[len - 1] == '\r') {
        len--;
    }
    return sw_string_from_literal(buffer, len);
}

int64_t seek(int64_t fd, int64_t offset, int64_t origin) {
    if (fd < 0 || fd >= SW_MAX_FILES || sw_files[fd] == NULL) {
        return -1;
    }
    return fseek(sw_files[fd], (long)offset, (int)origin) == 0 ? 0 : -1;
}

int64_t file_size(int64_t fd) {
    if (fd < 0 || fd >= SW_MAX_FILES || sw_files[fd] == NULL) {
        return -1;
    }
    fseek(sw_files[fd], 0, 2);
    long size = ftell(sw_files[fd]);
    rewind(sw_files[fd]);
    return size < 0 ? -1 : (int64_t)size;
}

int64_t exists(sw_string* path) {
#if defined(_WIN32)
    // fopen 无法打开目录，目录存在性必须走 GetFileAttributesA。
    extern unsigned int GetFileAttributesA(const char* path);
    return GetFileAttributesA(path->data) != 0xFFFFFFFFu ? 1 : 0;
#else
    sw_file_handle* file = fopen(path->data, "rb");
    if (file == NULL) {
        return 0;
    }
    fclose(file);
    return 1;
#endif
}

sw_string* path_join(sw_string* a, sw_string* b) {
#if defined(_WIN32)
    const char separator = '\\';
#else
    const char separator = '/';
#endif
    int64_t len = a->len + b->len + 1;
    char* buffer = (char*)sw_gc_alloc((uint64_t)len + 1);
    int64_t out = 0;
    for (int64_t i = 0; i < a->len; i++) {
        buffer[out++] = a->data[i];
    }
    if (a->len > 0 && a->data[a->len - 1] != separator) {
        buffer[out++] = separator;
    }
    for (int64_t i = 0; i < b->len; i++) {
        buffer[out++] = b->data[i];
    }
    buffer[out] = 0;
    return sw_string_from_literal(buffer, out);
}

sw_string* path_basename(sw_string* path) {
    int64_t last = -1;
    for (int64_t i = 0; i < path->len; i++) {
        if (path->data[i] == '/' || path->data[i] == '\\') {
            last = i;
        }
    }
    return sw_string_from_literal(path->data + last + 1, path->len - last - 1);
}

sw_string* path_dirname(sw_string* path) {
    int64_t last = -1;
    for (int64_t i = 0; i < path->len; i++) {
        if (path->data[i] == '/' || path->data[i] == '\\') {
            last = i;
        }
    }
    if (last < 0) {
        return sw_string_from_literal(".", 1);
    }
    return sw_string_from_literal(path->data, last);
}

sw_string* path_ext(sw_string* path) {
    int64_t last_sep = -1;
    int64_t last_dot = -1;
    for (int64_t i = 0; i < path->len; i++) {
        if (path->data[i] == '/' || path->data[i] == '\\') {
            last_sep = i;
            last_dot = -1;
        } else if (path->data[i] == '.') {
            last_dot = i;
        }
    }
    if (last_dot < 0 || last_dot <= last_sep) {
        return sw_string_from_literal("", 0);
    }
    return sw_string_from_literal(path->data + last_dot, path->len - last_dot);
}

int64_t is_dir(sw_string* path) {
#if defined(_WIN32)
    extern unsigned int GetFileAttributesA(const char* path);
    unsigned int attrs = GetFileAttributesA(path->data);
    return attrs != 0xFFFFFFFFu && (attrs & 0x10) ? 1 : 0;
#else
    extern void* opendir(const char* path);
    extern int closedir(void* dir);
    void* dir = opendir(path->data);
    if (dir == NULL) {
        return 0;
    }
    closedir(dir);
    return 1;
#endif
}

#if defined(_WIN32)
int64_t sw_mkdir(sw_string* path) {
    extern int CreateDirectoryA(const char* path, void* security);
    return CreateDirectoryA(path->data, NULL) ? 0 : -1;
}
#else
// 注意：不能把 libc 符号直接改名成 mkdir/rename/remove —— 那会与本文件导出的
// 同名包装函数冲突（ELF 上可执行文件定义会遮蔽 libc 同名函数，造成无限递归；
// Mach-O 上符号还带下划线前缀）。这里统一改用 mkdirat/renameat/unlink/rmdir，
// 三平台均有且与包装函数名不冲突。
#if defined(__APPLE__)
// Mach-O 的 C 符号带下划线前缀。
#define SW_LIBC_SYM(name) "_" name
#define SW_AT_FDCWD (-2)
#else
#define SW_LIBC_SYM(name) name
#define SW_AT_FDCWD (-100)
#endif

extern int sw_libc_mkdirat(int dirfd, const char* path, unsigned int mode)
    __asm__(SW_LIBC_SYM("mkdirat"));
extern int sw_libc_renameat(
    int old_dirfd,
    const char* old_path,
    int new_dirfd,
    const char* new_path
) __asm__(SW_LIBC_SYM("renameat"));
extern int sw_libc_unlink(const char* path) __asm__(SW_LIBC_SYM("unlink"));
extern int sw_libc_rmdir(const char* path) __asm__(SW_LIBC_SYM("rmdir"));

int64_t sw_mkdir(sw_string* path) {
    return sw_libc_mkdirat(SW_AT_FDCWD, path->data, 0755) == 0 ? 0 : -1;
}
#endif

#if defined(_WIN32)
int64_t sw_rename(sw_string* old_path, sw_string* new_path) {
    extern int MoveFileA(const char* old_path, const char* new_path);
    return MoveFileA(old_path->data, new_path->data) ? 0 : -1;
}

int64_t sw_remove(sw_string* path) {
    extern int DeleteFileA(const char* path);
    extern int RemoveDirectoryA(const char* path);
    if (DeleteFileA(path->data)) {
        return 0;
    }
    return RemoveDirectoryA(path->data) ? 0 : -1;
}
#else
int64_t sw_rename(sw_string* old_path, sw_string* new_path) {
    return sw_libc_renameat(
               SW_AT_FDCWD,
               old_path->data,
               SW_AT_FDCWD,
               new_path->data
           ) == 0
        ? 0
        : -1;
}

int64_t sw_remove(sw_string* path) {
    if (sw_libc_unlink(path->data) == 0) {
        return 0;
    }
    if (sw_libc_rmdir(path->data) == 0) {
        return 0;
    }
    return -1;
}
#endif

int64_t copy_file(sw_string* src, sw_string* dst) {
    sw_file_handle* in = fopen(src->data, "rb");
    if (in == NULL) {
        return -1;
    }
    sw_file_handle* out = fopen(dst->data, "wb");
    if (out == NULL) {
        fclose(in);
        return -1;
    }
    char buffer[8192];
    int64_t total = 0;
    while (1) {
        uint64_t read = fread(buffer, 1, sizeof(buffer), in);
        if (read == 0) {
            break;
        }
        fwrite(buffer, 1, read, out);
        total += (int64_t)read;
    }
    fclose(in);
    fclose(out);
    return total;
}

sw_array* sw_array_new(int64_t elem_size, int64_t count);

#if defined(_WIN32)
sw_array* list_dir(sw_string* path) {
    typedef struct {
        unsigned int attrs;
        unsigned char ctime[8];
        unsigned char atime[8];
        unsigned char wtime[8];
        unsigned int size_hi;
        unsigned int size_lo;
        unsigned int reserved0;
        unsigned int reserved1;
        char name[260];
        char alt_name[14];
    } sw_find_data;
    extern void* FindFirstFileA(const char* pattern, sw_find_data* data);
    extern int FindNextFileA(void* handle, sw_find_data* data);
    extern int FindClose(void* handle);

    char* pattern = (char*)sw_gc_alloc((uint64_t)path->len + 3);
    for (int64_t i = 0; i < path->len; i++) {
        pattern[i] = path->data[i];
    }
    pattern[path->len] = '\\';
    pattern[path->len + 1] = '*';
    pattern[path->len + 2] = 0;

    sw_find_data data;
    void* handle = FindFirstFileA(pattern, &data);
    if (handle == NULL || handle == (void*)-1) {
        return sw_array_new(8, 0);
    }
    sw_array* array = sw_array_new(8, 16);
    int64_t slot = 0;
    do {
        const char* name = data.name;
        if (name[0] == '.' && (name[1] == 0 || (name[1] == '.' && name[2] == 0))) {
            continue;
        }
        if (slot >= array->len) {
            sw_array* bigger = sw_array_new(8, array->len * 2 + 1);
            for (int64_t i = 0; i < slot; i++) {
                ((int64_t*)bigger->data)[i] = ((int64_t*)array->data)[i];
            }
            array = bigger;
        }
        ((int64_t*)array->data)[slot++] =
            (int64_t)sw_string_from_literal(name, (int64_t)strlen(name));
    } while (FindNextFileA(handle, &data));
    FindClose(handle);
    array->len = slot;
    array->cap = slot;
    return array;
}
#else
#if defined(__APPLE__)
#define SW_DIRENT_NAME_OFFSET 21
#else
#define SW_DIRENT_NAME_OFFSET 19
#endif

sw_array* list_dir(sw_string* path) {
    typedef struct {
        unsigned char raw[512];
    } sw_dirent;
    extern void* opendir(const char* path);
    extern sw_dirent* readdir(void* dir);
    extern int closedir(void* dir);

    void* dir = opendir(path->data);
    if (dir == NULL) {
        return sw_array_new(8, 0);
    }
    sw_array* array = sw_array_new(8, 16);
    int64_t slot = 0;
    while (1) {
        sw_dirent* entry = readdir(dir);
        if (entry == NULL) {
            break;
        }
        const char* name = (const char*)entry + SW_DIRENT_NAME_OFFSET;
        if (name[0] == '.' && (name[1] == 0 || (name[1] == '.' && name[2] == 0))) {
            continue;
        }
        if (slot >= array->len) {
            sw_array* bigger = sw_array_new(8, array->len * 2 + 1);
            for (int64_t i = 0; i < slot; i++) {
                ((int64_t*)bigger->data)[i] = ((int64_t*)array->data)[i];
            }
            array = bigger;
        }
        ((int64_t*)array->data)[slot++] =
            (int64_t)sw_string_from_literal(name, (int64_t)strlen(name));
    }
    closedir(dir);
    array->len = slot;
    array->cap = slot;
    return array;
}
#endif

static void sw_walk_impl(
    sw_string* base,
    int64_t want_dirs,
    sw_array** array,
    int64_t* slot,
    int64_t* capacity
) {
    sw_array* entries = list_dir(base);
    for (int64_t i = 0; i < entries->len; i++) {
        sw_string* name = (sw_string*)((int64_t*)entries->data)[i];
        sw_string* full = path_join(base, name);
        if (is_dir(full)) {
            if (want_dirs) {
                if (*slot >= *capacity) {
                    *capacity = *capacity * 2 + 1;
                    sw_array* bigger = sw_array_new(8, *capacity);
                    for (int64_t j = 0; j < *slot; j++) {
                        ((int64_t*)bigger->data)[j] = ((int64_t*)(*array)->data)[j];
                    }
                    *array = bigger;
                }
                ((int64_t*)(*array)->data)[(*slot)++] = (int64_t)full;
            }
            sw_walk_impl(full, want_dirs, array, slot, capacity);
        } else if (!want_dirs) {
            if (*slot >= *capacity) {
                *capacity = *capacity * 2 + 1;
                sw_array* bigger = sw_array_new(8, *capacity);
                for (int64_t j = 0; j < *slot; j++) {
                    ((int64_t*)bigger->data)[j] = ((int64_t*)(*array)->data)[j];
                }
                *array = bigger;
            }
            ((int64_t*)(*array)->data)[(*slot)++] = (int64_t)full;
        }
    }
}

static sw_array* sw_walk(sw_string* path, int64_t want_dirs) {
    sw_array* array = sw_array_new(8, 16);
    int64_t slot = 0;
    int64_t capacity = 16;
    sw_walk_impl(path, want_dirs, &array, &slot, &capacity);
    array->len = slot;
    array->cap = slot;
    return array;
}

sw_array* walk_files(sw_string* path) {
    return sw_walk(path, 0);
}

sw_array* walk_dirs(sw_string* path) {
    return sw_walk(path, 1);
}

sw_string* read_all(sw_string* path) {
    sw_file_handle* file = fopen(path->data, "rb");
    if (file == NULL) {
        return sw_string_from_literal("", 0);
    }
    fseek(file, 0, 2);
    long size = ftell(file);
    rewind(file);
    if (size < 0) {
        size = 0;
    }
    sw_string* string =
        (sw_string*)sw_gc_alloc(sizeof(sw_string) + (uint64_t)size + 1);
    char* data = (char*)(string + 1);
    if (size > 0) {
        fread(data, 1, (uint64_t)size, file);
    }
    data[size] = 0;
    fclose(file);
    string->data = data;
    string->len = size;
    return string;
}

sw_array* sw_array_new(int64_t elem_size, int64_t count);

sw_array* read_lines(sw_string* path) {
    sw_string* content = read_all(path);
    if (content->len == 0) {
        return sw_array_new(8, 0);
    }
    int64_t capacity = 1;
    for (int64_t i = 0; i < content->len; i++) {
        if (content->data[i] == '\n') {
            capacity++;
        }
    }
    sw_array* array = sw_array_new(8, capacity);
    int64_t slot = 0;
    int64_t start = 0;
    for (int64_t i = 0; i < content->len; i++) {
        if (content->data[i] == '\n') {
            int64_t len = i - start;
            if (len > 0 && content->data[start + len - 1] == '\r') {
                len--;
            }
            ((int64_t*)array->data)[slot++] =
                (int64_t)sw_string_from_literal(content->data + start, len);
            start = i + 1;
        }
    }
    int64_t len = content->len - start;
    if (len > 0) {
        if (content->data[start + len - 1] == '\r') {
            len--;
        }
        ((int64_t*)array->data)[slot++] =
            (int64_t)sw_string_from_literal(content->data + start, len);
    }
    array->len = slot;
    array->cap = slot;
    return array;
}

int64_t write_all(sw_string* path, sw_string* text) {
    sw_file_handle* file = fopen(path->data, "wb");
    if (file == NULL) {
        return -1;
    }
    int64_t written = (int64_t)fwrite(text->data, 1, (uint64_t)text->len, file);
    fclose(file);
    return written;
}

int64_t append(sw_string* path, sw_string* text) {
    sw_file_handle* file = fopen(path->data, "ab");
    if (file == NULL) {
        return -1;
    }
    int64_t written = (int64_t)fwrite(text->data, 1, (uint64_t)text->len, file);
    fclose(file);
    return written;
}

// ---------------------------------------------------------------------------
// string：字节语义的查找/子串（UTF-8 边界由用户保证，与 .length 一致）。
// ---------------------------------------------------------------------------

int64_t string_eq(sw_string* a, sw_string* b) {
    if (a->len != b->len) {
        return 0;
    }
    return memcmp(a->data, b->data, (uint64_t)a->len) == 0 ? 1 : 0;
}

int64_t string_ne(sw_string* a, sw_string* b) {
    return string_eq(a, b) ? 0 : 1;
}

int64_t index_of(sw_string* text, sw_string* needle) {
    if (needle->len == 0) {
        return 0;
    }
    if (needle->len > text->len) {
        return -1;
    }
    for (int64_t start = 0; start + needle->len <= text->len; start++) {
        int64_t match = 1;
        for (int64_t offset = 0; offset < needle->len; offset++) {
            if (text->data[start + offset] != needle->data[offset]) {
                match = 0;
                break;
            }
        }
        if (match) {
            return start;
        }
    }
    return -1;
}

int64_t contains(sw_string* text, sw_string* needle) {
    return index_of(text, needle) >= 0 ? 1 : 0;
}

int64_t starts_with(sw_string* text, sw_string* prefix) {
    if (prefix->len > text->len) {
        return 0;
    }
    for (int64_t index = 0; index < prefix->len; index++) {
        if (text->data[index] != prefix->data[index]) {
            return 0;
        }
    }
    return 1;
}

sw_string* substring(sw_string* text, int64_t start, int64_t length) {
    if (start < 0 || start > text->len || length < 0) {
        return sw_string_from_literal("", 0);
    }
    if (start + length > text->len) {
        length = text->len - start;
    }
    return sw_string_from_literal(text->data + start, length);
}

extern long long strtoll(const char* text, char** end, int base);
extern double strtod(const char* text, char** end);

int64_t parse_int(sw_string* text) {
    return strtoll(text->data, NULL, 10);
}

double parse_float(sw_string* text) {
    return strtod(text->data, NULL);
}

int64_t is_number(sw_string* text) {
    int64_t index = 0;
    if (index < text->len && (text->data[index] == '+' || text->data[index] == '-')) {
        index++;
    }
    int64_t digits = 0;
    while (index < text->len && text->data[index] >= '0' && text->data[index] <= '9') {
        index++;
        digits++;
    }
    if (index < text->len && text->data[index] == '.') {
        index++;
        while (index < text->len && text->data[index] >= '0' && text->data[index] <= '9') {
            index++;
            digits++;
        }
    }
    if (digits == 0) {
        return 0;
    }
    if (index < text->len && (text->data[index] == 'e' || text->data[index] == 'E')) {
        index++;
        if (index < text->len && (text->data[index] == '+' || text->data[index] == '-')) {
            index++;
        }
        int64_t exp_digits = 0;
        while (index < text->len && text->data[index] >= '0' && text->data[index] <= '9') {
            index++;
            exp_digits++;
        }
        if (exp_digits == 0) {
            return 0;
        }
    }
    return index == text->len ? 1 : 0;
}

static char sw_ascii_lower(char c) {
    return (c >= 'A' && c <= 'Z') ? (char)(c + 32) : c;
}

static int sw_text_equals_ci(sw_string* text, const char* literal) {
    if (text == NULL || literal == NULL) {
        return 0;
    }
    const char* data = text->data;
    int64_t len = text->len;
    for (int64_t i = 0; literal[i] != 0; i++) {
        if (i >= len || sw_ascii_lower(data[i]) != literal[i]) {
            return 0;
        }
    }
    return len > 0 && literal[len] == 0;
}

int64_t parse_bool(sw_string* text) {
    if (text == NULL) {
        return 0;
    }
    if (sw_text_equals_ci(text, "true") || sw_text_equals_ci(text, "1") ||
        sw_text_equals_ci(text, "yes")) {
        return 1;
    }
    if (sw_text_equals_ci(text, "false") || sw_text_equals_ci(text, "0") ||
        sw_text_equals_ci(text, "no")) {
        return 0;
    }
    return 0;
}

int64_t parse_int_or(sw_string* text, int64_t fallback) {
    char* end = NULL;
    long long value = strtoll(text->data, &end, 10);
    if (end == text->data || *end != 0) {
        return fallback;
    }
    return (int64_t)value;
}

double parse_float_or(sw_string* text, double fallback) {
    char* end = NULL;
    double value = strtod(text->data, &end);
    if (end == text->data || *end != 0) {
        return fallback;
    }
    return value;
}

sw_string* repeat(sw_string* text, int64_t count) {
    if (count <= 0 || text->len == 0) {
        return sw_string_from_literal("", 0);
    }
    int64_t total = text->len * count;
    char* buffer = (char*)sw_gc_alloc((uint64_t)total + 1);
    for (int64_t i = 0; i < count; i++) {
        memcpy(buffer + i * text->len, text->data, (sw_size)(uint64_t)text->len);
    }
    buffer[total] = 0;
    return sw_string_from_literal(buffer, total);
}

sw_string* from_code_point(int64_t cp) {
    char buffer[5];
    int64_t len = 0;
    if (cp < 0x80) {
        buffer[len++] = (char)cp;
    } else if (cp < 0x800) {
        buffer[len++] = (char)(0xC0 | (cp >> 6));
        buffer[len++] = (char)(0x80 | (cp & 0x3F));
    } else if (cp < 0x10000) {
        buffer[len++] = (char)(0xE0 | (cp >> 12));
        buffer[len++] = (char)(0x80 | ((cp >> 6) & 0x3F));
        buffer[len++] = (char)(0x80 | (cp & 0x3F));
    } else {
        buffer[len++] = (char)(0xF0 | (cp >> 18));
        buffer[len++] = (char)(0x80 | ((cp >> 12) & 0x3F));
        buffer[len++] = (char)(0x80 | ((cp >> 6) & 0x3F));
        buffer[len++] = (char)(0x80 | (cp & 0x3F));
    }
    buffer[len] = 0;
    return sw_string_from_literal(buffer, len);
}

sw_string* to_upper(sw_string* text) {
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len + 1);
    for (int64_t i = 0; i < text->len; i++) {
        char c = text->data[i];
        buffer[i] = (c >= 'a' && c <= 'z') ? (char)(c - 32) : c;
    }
    buffer[text->len] = 0;
    return sw_string_from_literal(buffer, text->len);
}

sw_string* to_lower(sw_string* text) {
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len + 1);
    for (int64_t i = 0; i < text->len; i++) {
        char c = text->data[i];
        buffer[i] = (c >= 'A' && c <= 'Z') ? (char)(c + 32) : c;
    }
    buffer[text->len] = 0;
    return sw_string_from_literal(buffer, text->len);
}

sw_string* trim(sw_string* text) {
    int64_t start = 0;
    int64_t end = text->len;
    while (start < end &&
           (text->data[start] == ' ' || text->data[start] == '\t' ||
            text->data[start] == '\n' || text->data[start] == '\r')) {
        start++;
    }
    while (end > start &&
           (text->data[end - 1] == ' ' || text->data[end - 1] == '\t' ||
            text->data[end - 1] == '\n' || text->data[end - 1] == '\r')) {
        end--;
    }
    return sw_string_from_literal(text->data + start, end - start);
}

sw_array* sw_array_new(int64_t elem_size, int64_t count);

sw_array* split(sw_string* text, sw_string* sep) {
    if (sep->len == 0) {
        sw_array* single = sw_array_new(8, 1);
        ((int64_t*)single->data)[0] = (int64_t)sw_string_from_literal(text->data, text->len);
        return single;
    }
    int64_t count = 1;
    for (int64_t i = 0; i + sep->len <= text->len; i++) {
        int64_t match = 1;
        for (int64_t j = 0; j < sep->len; j++) {
            if (text->data[i + j] != sep->data[j]) {
                match = 0;
                break;
            }
        }
        if (match) {
            count++;
            i += sep->len - 1;
        }
    }
    sw_array* array = sw_array_new(8, count);
    int64_t start = 0;
    int64_t slot = 0;
    for (int64_t i = 0; i + sep->len <= text->len; i++) {
        int64_t match = 1;
        for (int64_t j = 0; j < sep->len; j++) {
            if (text->data[i + j] != sep->data[j]) {
                match = 0;
                break;
            }
        }
        if (match) {
            ((int64_t*)array->data)[slot++] =
                (int64_t)sw_string_from_literal(text->data + start, i - start);
            i += sep->len - 1;
            start = i + 1;
        }
    }
    ((int64_t*)array->data)[slot] =
        (int64_t)sw_string_from_literal(text->data + start, text->len - start);
    return array;
}

sw_string* join(sw_array* array, sw_string* sep) {
    int64_t total = 0;
    for (int64_t i = 0; i < array->len; i++) {
        sw_string* item = (sw_string*)((int64_t*)array->data)[i];
        total += item->len;
        if (i > 0) {
            total += sep->len;
        }
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)total + 1);
    int64_t out = 0;
    for (int64_t i = 0; i < array->len; i++) {
        sw_string* item = (sw_string*)((int64_t*)array->data)[i];
        if (i > 0) {
            for (int64_t j = 0; j < sep->len; j++) {
                buffer[out++] = sep->data[j];
            }
        }
        for (int64_t j = 0; j < item->len; j++) {
            buffer[out++] = item->data[j];
        }
    }
    buffer[out] = 0;
    return sw_string_from_literal(buffer, total);
}

sw_string* replace(sw_string* text, sw_string* from, sw_string* to) {
    if (from->len == 0) {
        return sw_string_from_literal(text->data, text->len);
    }
    int64_t count = 0;
    for (int64_t i = 0; i + from->len <= text->len; i++) {
        int64_t match = 1;
        for (int64_t j = 0; j < from->len; j++) {
            if (text->data[i + j] != from->data[j]) {
                match = 0;
                break;
            }
        }
        if (match) {
            count++;
            i += from->len - 1;
        }
    }
    int64_t total = text->len + count * (to->len - from->len);
    char* buffer = (char*)sw_gc_alloc((uint64_t)total + 1);
    int64_t out = 0;
    for (int64_t i = 0; i < text->len;) {
        int64_t match = i + from->len <= text->len;
        if (match) {
            for (int64_t j = 0; j < from->len; j++) {
                if (text->data[i + j] != from->data[j]) {
                    match = 0;
                    break;
                }
            }
        }
        if (match) {
            for (int64_t j = 0; j < to->len; j++) {
                buffer[out++] = to->data[j];
            }
            i += from->len;
        } else {
            buffer[out++] = text->data[i];
            i++;
        }
    }
    buffer[out] = 0;
    return sw_string_from_literal(buffer, total);
}

// ---------------------------------------------------------------------------
// 字符级（UTF-8 码点）字符串操作
// ---------------------------------------------------------------------------

int64_t utf8_len(sw_string* text);
static int64_t sw_utf8_char_length(const char* text, int64_t index, int64_t len);

sw_string* reverse(sw_string* text) {
    int64_t count = utf8_len(text);
    if (count <= 1) {
        return sw_string_from_literal(text->data, text->len);
    }
    int64_t* starts = (int64_t*)sw_gc_alloc((uint64_t)(count + 1) * 8);
    int64_t byte = 0;
    int64_t n = 0;
    while (byte < text->len) {
        starts[n++] = byte;
        byte += sw_utf8_char_length(text->data, byte, text->len);
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len + 1);
    int64_t out = 0;
    for (int64_t i = n - 1; i >= 0; i--) {
        int64_t len = sw_utf8_char_length(text->data, starts[i], text->len);
        for (int64_t j = 0; j < len; j++) {
            buffer[out++] = text->data[starts[i] + j];
        }
    }
    buffer[out] = 0;
    return sw_string_from_literal(buffer, out);
}

int64_t index_of_char(sw_string* text, sw_string* needle) {
    if (needle->len == 0) {
        return 0;
    }
    if (needle->len > text->len) {
        return -1;
    }
    int64_t byte = 0;
    int64_t char_index = 0;
    while (byte + needle->len <= text->len) {
        int64_t ok = 1;
        for (int64_t j = 0; j < needle->len; j++) {
            if (text->data[byte + j] != needle->data[j]) {
                ok = 0;
                break;
            }
        }
        if (ok) {
            return char_index;
        }
        byte += sw_utf8_char_length(text->data, byte, text->len);
        char_index++;
    }
    return -1;
}

sw_array* split_chars(sw_string* text, sw_string* sep) {
    // UTF-8 有效分隔符的字节序列天然落在字符边界，与 split 行为一致；
    // 单独提供以明确"按字符"语义（避免与字节偏移函数混淆）。
    return split(text, sep);
}

// ---------------------------------------------------------------------------
// 格式化输出：补零/对齐/精度
// ---------------------------------------------------------------------------

static int64_t sw_pad_char_len(sw_string* pad) {
    return pad->len > 0 ? sw_utf8_char_length(pad->data, 0, pad->len) : 1;
}

sw_string* pad_left(sw_string* text, int64_t width, sw_string* pad) {
    int64_t text_chars = utf8_len(text);
    int64_t need = width - text_chars;
    if (need <= 0) {
        return sw_string_from_literal(text->data, text->len);
    }
    const char* pad_bytes = pad->len > 0 ? pad->data : " ";
    int64_t pad_len = sw_pad_char_len(pad);
    int64_t total = text->len + need * pad_len;
    char* buffer = (char*)sw_gc_alloc((uint64_t)total + 1);
    int64_t out = 0;
    for (int64_t i = 0; i < need; i++) {
        for (int64_t j = 0; j < pad_len; j++) {
            buffer[out++] = pad_bytes[j];
        }
    }
    for (int64_t i = 0; i < text->len; i++) {
        buffer[out++] = text->data[i];
    }
    buffer[out] = 0;
    return sw_string_from_literal(buffer, out);
}

sw_string* pad_right(sw_string* text, int64_t width, sw_string* pad) {
    int64_t text_chars = utf8_len(text);
    int64_t need = width - text_chars;
    if (need <= 0) {
        return sw_string_from_literal(text->data, text->len);
    }
    const char* pad_bytes = pad->len > 0 ? pad->data : " ";
    int64_t pad_len = sw_pad_char_len(pad);
    int64_t total = text->len + need * pad_len;
    char* buffer = (char*)sw_gc_alloc((uint64_t)total + 1);
    int64_t out = 0;
    for (int64_t i = 0; i < text->len; i++) {
        buffer[out++] = text->data[i];
    }
    for (int64_t i = 0; i < need; i++) {
        for (int64_t j = 0; j < pad_len; j++) {
            buffer[out++] = pad_bytes[j];
        }
    }
    buffer[out] = 0;
    return sw_string_from_literal(buffer, out);
}

sw_string* format_int(int64_t value, int64_t width, int64_t pad_zero) {
    char buffer[32];
    int len;
    if (pad_zero) {
        len = snprintf(buffer, sizeof(buffer), "%0*lld", (int)width, value);
    } else {
        len = snprintf(buffer, sizeof(buffer), "%*lld", (int)width, value);
    }
    return sw_string_from_literal(buffer, len);
}

sw_string* format_float(double value, int64_t precision) {
    char buffer[64];
    int len = snprintf(buffer, sizeof(buffer), "%.*f", (int)precision, value);
    return sw_string_from_literal(buffer, len);
}

// ---------------------------------------------------------------------------
// 随机数与数学小工具
// ---------------------------------------------------------------------------

extern int rand(void);
extern void srand(unsigned int seed);

static int sw_rand_seeded = 0;

int64_t now_ms(void);

int64_t rand_int(int64_t max) {
    if (!sw_rand_seeded) {
        srand((unsigned int)(now_ms() ^ (uintptr_t)&sw_rand_seeded));
        sw_rand_seeded = 1;
    }
    if (max <= 0) {
        return 0;
    }
    return (int64_t)(rand() % (int)max);
}

int64_t clamp(int64_t value, int64_t lo, int64_t hi) {
    return value < lo ? lo : (value > hi ? hi : value);
}

int64_t gcd(int64_t a, int64_t b) {
    if (a < 0) {
        a = -a;
    }
    if (b < 0) {
        b = -b;
    }
    while (b != 0) {
        int64_t t = a % b;
        a = b;
        b = t;
    }
    return a;
}

int64_t lcm(int64_t a, int64_t b) {
    if (a == 0 || b == 0) {
        return 0;
    }
    return a / gcd(a, b) * b;
}

// ---------------------------------------------------------------------------
// unicode：UTF-8 按字符（码点）语义的工具函数。
// ---------------------------------------------------------------------------

static int64_t sw_utf8_char_length(const char* text, int64_t index, int64_t len) {
    unsigned char first = (unsigned char)text[index];
    if (first < 0x80) {
        return 1;
    }
    if ((first & 0xE0) == 0xC0 && index + 1 < len) {
        return 2;
    }
    if ((first & 0xF0) == 0xE0 && index + 2 < len) {
        return 3;
    }
    if ((first & 0xF8) == 0xF0 && index + 3 < len) {
        return 4;
    }
    return 1;
}

static int64_t sw_utf8_decode(const char* text, int64_t index, int64_t char_len) {
    unsigned char first = (unsigned char)text[index];
    if (char_len == 1) {
        return first;
    }
    if (char_len == 2) {
        return ((first & 0x1F) << 6) | ((unsigned char)text[index + 1] & 0x3F);
    }
    if (char_len == 3) {
        return ((first & 0x0F) << 12) | (((unsigned char)text[index + 1] & 0x3F) << 6) |
               ((unsigned char)text[index + 2] & 0x3F);
    }
    return ((first & 0x07) << 18) | (((unsigned char)text[index + 1] & 0x3F) << 12) |
           (((unsigned char)text[index + 2] & 0x3F) << 6) |
           ((unsigned char)text[index + 3] & 0x3F);
}

int64_t utf8_len(sw_string* text) {
    int64_t count = 0;
    int64_t index = 0;
    while (index < text->len) {
        index += sw_utf8_char_length(text->data, index, text->len);
        count++;
    }
    return count;
}

int64_t utf8_char_at(sw_string* text, int64_t index) {
    int64_t position = 0;
    int64_t offset = 0;
    while (offset < text->len && position < index) {
        offset += sw_utf8_char_length(text->data, offset, text->len);
        position++;
    }
    if (offset >= text->len) {
        return -1;
    }
    int64_t char_len = sw_utf8_char_length(text->data, offset, text->len);
    return sw_utf8_decode(text->data, offset, char_len);
}

sw_string* utf8_substring(sw_string* text, int64_t start, int64_t count) {
    int64_t offset = 0;
    int64_t position = 0;
    while (offset < text->len && position < start) {
        offset += sw_utf8_char_length(text->data, offset, text->len);
        position++;
    }
    if (position < start) {
        return sw_string_from_literal("", 0);
    }
    int64_t end = offset;
    int64_t taken = 0;
    while (end < text->len && taken < count) {
        end += sw_utf8_char_length(text->data, end, text->len);
        taken++;
    }
    return sw_string_from_literal(text->data + offset, end - offset);
}

int64_t utf8_byte_len(sw_string* text) {
    if (text == NULL) {
        return 0;
    }
    return text->len;
}

/// 第 char_index 个字符的字节起始偏移；越界返回 -1。
int64_t utf8_index_to_byte(sw_string* text, int64_t char_index) {
    int64_t position = 0;
    int64_t offset = 0;
    while (offset < text->len && position < char_index) {
        offset += sw_utf8_char_length(text->data, offset, text->len);
        position++;
    }
    if (offset < text->len && position == char_index) {
        return offset;
    }
    if (position == char_index && offset == text->len) {
        // 恰好指向字符串末尾（可用于 append 位置）。
        return text->len;
    }
    return -1;
}

/// byte_offset 所在字符的字符序号；offset 落在多字节字符中间或越界返回 -1。
int64_t utf8_byte_to_index(sw_string* text, int64_t byte_offset) {
    if (byte_offset < 0 || byte_offset > text->len) {
        return -1;
    }
    if (byte_offset == text->len) {
        return utf8_len(text);
    }
    int64_t position = 0;
    int64_t offset = 0;
    while (offset < text->len) {
        if (offset == byte_offset) {
            return position;
        }
        int64_t char_len = sw_utf8_char_length(text->data, offset, text->len);
        offset += char_len;
        position++;
    }
    return -1;
}

/// 是否全部为可打印字符（普通 ASCII 可见 + 非 ASCII UTF-8 序列；控制字符算不可打印）。
int64_t utf8_is_printable(sw_string* text) {
    int64_t index = 0;
    while (index < text->len) {
        unsigned char byte = (unsigned char)text->data[index];
        if (byte < 0x80) {
            // ASCII：可打印区间 0x20-0x7E；\n \t \r 视为可打印（文本行长用）。
            if (!((byte >= 0x20 && byte <= 0x7E) || byte == '\n' || byte == '\t' || byte == '\r')) {
                return 0;
            }
            index += 1;
        } else {
            int64_t char_len = sw_utf8_char_length(text->data, index, text->len);
            int64_t cp = sw_utf8_decode(text->data, index, char_len);
            if (cp < 0 || (cp >= 0xE000 && cp <= 0xF8FF)) {
                // 排除私有区段（通常不可打印）。
                return 0;
            }
            index += char_len;
        }
    }
    return 1;
}

// 码点是否为 CJK 字符（中日韩统一表意文字/扩展 A/B/C/D/E、兼容表意、
// 假名、谚文）。
static int sw_codepoint_is_cjk(int64_t cp) {
    return (cp >= 0x4E00 && cp <= 0x9FFF) ||   // 统一表意文字
           (cp >= 0x3400 && cp <= 0x4DBF) ||   // 扩展 A
           (cp >= 0x20000 && cp <= 0x2A6DF) || // 扩展 B
           (cp >= 0x2A700 && cp <= 0x2B73F) || // 扩展 C
           (cp >= 0x2B740 && cp <= 0x2B81F) || // 扩展 D
           (cp >= 0x2B820 && cp <= 0x2CEAF) || // 扩展 E
           (cp >= 0xF900 && cp <= 0xFAFF) ||   // 兼容表意
           (cp >= 0x2F800 && cp <= 0x2FA1F) || // 兼容表意补充
           (cp >= 0x3040 && cp <= 0x30FF) ||   // 假名（平/片）
           (cp >= 0x31F0 && cp <= 0x31FF) ||   // 片假名扩展
           (cp >= 0xAC00 && cp <= 0xD7AF) ||   // 谚文音节
           (cp >= 0x1100 && cp <= 0x11FF);     // 谚文字母
}

// 码点是否为字母（Unicode 字母类：拉丁/希腊/西里尔/扩展拉丁，
// 以及 CJK 表意文字——Unicode 归类为 Letter Other (Lo)，面向中文用户
// 视为字母）。
static int sw_codepoint_is_letter(int64_t cp) {
    if (cp >= 'a' && cp <= 'z') {
        return 1;
    }
    if (cp >= 'A' && cp <= 'Z') {
        return 1;
    }
    if (sw_codepoint_is_cjk(cp)) {
        return 1;
    }
    return (cp >= 0x00C0 && cp <= 0x02AF) ||   // 拉丁-1 补充 + 拉丁扩展 A/B
           (cp >= 0x0370 && cp <= 0x03FF) ||   // 希腊
           (cp >= 0x0400 && cp <= 0x052F) ||   // 西里尔 + 补充
           (cp >= 0x1E00 && cp <= 0x1EFF) ||   // 拉丁扩展附加
           (cp >= 0x2C60 && cp <= 0x2C7F) ||   // 拉丁扩展 C
           (cp >= 0xA720 && cp <= 0xA7FF) ||   // 拉丁扩展 D
           (cp >= 0x2DE0 && cp <= 0x2DFF) ||   // 西里尔扩展 A
           (cp >= 0xA640 && cp <= 0xA69F) ||   // 西里尔扩展 B
           (cp >= 0x0100 && cp <= 0x017F);     // 拉丁扩展-A（含 ü/é 等）
}

// 码点是否为数字（ASCII + 全角 + 阿拉伯-印度数字等）。
static int sw_codepoint_is_digit(int64_t cp) {
    return (cp >= '0' && cp <= '9') ||
           (cp >= 0xFF10 && cp <= 0xFF19) ||   // 全角数字
           (cp >= 0x0660 && cp <= 0x0669) ||   // 阿拉伯-印度数字
           (cp >= 0x06F0 && cp <= 0x06F9) ||   // 扩展阿拉伯-印度数字
           (cp >= 0x0966 && cp <= 0x096F) ||   // 天城文数字
           (cp >= 0x0E50 && cp <= 0x0E59) ||   // 泰文数字
           (cp >= 0x0ED0 && cp <= 0x0ED9) ||   // 老挝数字
           (cp >= 0x3007 && cp <= 0x3007) ||   // 〇
           (cp >= 0x00B2 && cp <= 0x00B3) ||   // ² ³
           (cp >= 0x00BC && cp <= 0x00BE);     // ¼ ½ ¾
}

// 是否全部字符为 CJK（空串/含非 CJK 返回 false）。
int64_t sw_is_cjk(sw_string* text) {
    if (text == NULL || text->len == 0) {
        return 0;
    }
    int64_t index = 0;
    while (index < text->len) {
        int64_t char_len = sw_utf8_char_length(text->data, index, text->len);
        int64_t cp = sw_utf8_decode(text->data, index, char_len);
        if (cp < 0 || !sw_codepoint_is_cjk(cp)) {
            return 0;
        }
        index += char_len;
    }
    return 1;
}

// 是否全部字符为字母（空串/含非字母返回 false）。
int64_t sw_is_letter(sw_string* text) {
    if (text == NULL || text->len == 0) {
        return 0;
    }
    int64_t index = 0;
    while (index < text->len) {
        int64_t char_len = sw_utf8_char_length(text->data, index, text->len);
        int64_t cp = sw_utf8_decode(text->data, index, char_len);
        if (cp < 0 || !sw_codepoint_is_letter(cp)) {
            return 0;
        }
        index += char_len;
    }
    return 1;
}

// 是否全部字符为数字（空串/含非数字返回 false）。
int64_t sw_is_digit(sw_string* text) {
    if (text == NULL || text->len == 0) {
        return 0;
    }
    int64_t index = 0;
    while (index < text->len) {
        int64_t char_len = sw_utf8_char_length(text->data, index, text->len);
        int64_t cp = sw_utf8_decode(text->data, index, char_len);
        if (cp < 0 || !sw_codepoint_is_digit(cp)) {
            return 0;
        }
        index += char_len;
    }
    return 1;
}

// 字符显示宽度：CJK/全角宽字符记 2，其余记 1（终端对齐用）。
static int sw_codepoint_width(int64_t cp) {
    if (sw_codepoint_is_cjk(cp)) {
        return 2;
    }
    return (cp >= 0xFF01 && cp <= 0xFF60) ||   // 全角标点/符号
                   (cp >= 0x3000 && cp <= 0x303F) ||   // CJK 标点
                   (cp >= 0x2018 && cp <= 0x201F) ||   // 引号
                   (cp >= 0x3001 && cp <= 0x3002)
               ? 2
               : 1;
}

// 字符串总显示宽度（每字符 CJK/全角=2，其余=1）。
int64_t sw_char_width(sw_string* text) {
    if (text == NULL) {
        return 0;
    }
    int64_t total = 0;
    int64_t index = 0;
    while (index < text->len) {
        int64_t char_len = sw_utf8_char_length(text->data, index, text->len);
        int64_t cp = sw_utf8_decode(text->data, index, char_len);
        if (cp >= 0) {
            total += sw_codepoint_width(cp);
        }
        index += char_len;
    }
    return total;
}

// ---------------------------------------------------------------------------
// time：毫秒时间戳与睡眠。
// ---------------------------------------------------------------------------

#if defined(_WIN32)
extern void GetSystemTimeAsFileTime(void* file_time);
extern void Sleep(unsigned long milliseconds);

int64_t now_ms(void) {
    unsigned char ft[8];
    GetSystemTimeAsFileTime(ft);
    uint64_t since_1601 =
        ((uint64_t)(*(unsigned int*)(ft + 4)) << 32) | (*(unsigned int*)ft);
    // 1601-01-01 到 1970-01-01 的 100ns 间隔数。
    uint64_t unix_100ns = since_1601 - 116444736000000000ULL;
    return (int64_t)(unix_100ns / 10000);
}
#else
extern int clock_gettime(int clock_id, void* timespec);
extern int nanosleep(const void* req, void* rem);

int64_t now_ms(void) {
    unsigned char ts[16];
    clock_gettime(0, ts);
    int64_t seconds = *(int64_t*)ts;
    int64_t nanos = *(int64_t*)(ts + 8);
    return seconds * 1000 + nanos / 1000000;
}
#endif

int64_t now_sec(void) {
    return now_ms() / 1000;
}

#if defined(_WIN32)
static void sw_unix_to_local_systemtime(unsigned char* st, int64_t seconds) {
    extern int FileTimeToLocalFileTime(const void* file_time, void* local_file_time);
    extern int FileTimeToSystemTime(const void* file_time, void* system_time);
    uint64_t since_1601 = ((uint64_t)seconds + 11644473600ULL) * 10000000ULL;
    unsigned char ft[8];
    unsigned char local_ft[8];
    *(unsigned int*)ft = (unsigned int)since_1601;
    *(unsigned int*)(ft + 4) = (unsigned int)(since_1601 >> 32);
    FileTimeToLocalFileTime(ft, local_ft);
    FileTimeToSystemTime(local_ft, st);
}
#endif

sw_string* date_string(int64_t seconds) {
    char buffer[32];
#if defined(_WIN32)
    unsigned char st[16];
    sw_unix_to_local_systemtime(st, seconds);
    int year = *(unsigned short*)st;
    int month = *(unsigned short*)(st + 2);
    int day = *(unsigned short*)(st + 6);
    snprintf(buffer, sizeof(buffer), "%04d-%02d-%02d", year, month, day);
#else
    extern void* localtime_r(const void* time, void* tm);
    unsigned char tm[64];
    unsigned char t[8];
    *(int64_t*)t = seconds;
    if (localtime_r(t, tm) == NULL) {
        return sw_string_from_literal("", 0);
    }
    int year = *(int*)(tm + 20) + 1900;
    int month = *(int*)(tm + 16) + 1;
    int day = *(int*)(tm + 12);
    snprintf(buffer, sizeof(buffer), "%04d-%02d-%02d", year, month, day);
#endif
    return sw_string_from_literal(buffer, (int64_t)strlen(buffer));
}

sw_string* datetime_string(int64_t seconds) {
    char buffer[40];
#if defined(_WIN32)
    unsigned char st[16];
    sw_unix_to_local_systemtime(st, seconds);
    int year = *(unsigned short*)st;
    int month = *(unsigned short*)(st + 2);
    int day = *(unsigned short*)(st + 6);
    int hour = *(unsigned short*)(st + 8);
    int minute = *(unsigned short*)(st + 10);
    int second = *(unsigned short*)(st + 12);
    snprintf(
        buffer,
        sizeof(buffer),
        "%04d-%02d-%02d %02d:%02d:%02d",
        year,
        month,
        day,
        hour,
        minute,
        second
    );
#else
    extern void* localtime_r(const void* time, void* tm);
    unsigned char tm[64];
    unsigned char t[8];
    *(int64_t*)t = seconds;
    if (localtime_r(t, tm) == NULL) {
        return sw_string_from_literal("", 0);
    }
    int year = *(int*)(tm + 20) + 1900;
    int month = *(int*)(tm + 16) + 1;
    int day = *(int*)(tm + 12);
    int hour = *(int*)(tm + 8);
    int minute = *(int*)(tm + 4);
    int second = *(int*)(tm + 0);
    snprintf(
        buffer,
        sizeof(buffer),
        "%04d-%02d-%02d %02d:%02d:%02d",
        year,
        month,
        day,
        hour,
        minute,
        second
    );
#endif
    return sw_string_from_literal(buffer, (int64_t)strlen(buffer));
}

static int sw_format_conv(char c) {
    return c == 'd' || c == 'i' || c == 'u' || c == 'x' || c == 'X' ||
           c == 'o' || c == 'f' || c == 'e' || c == 'g' || c == 's' ||
           c == 'c';
}

static char* sw_format_grow(char* buffer, int64_t* cap, int64_t used, int64_t needed) {
    if (used + needed + 1 <= *cap) {
        return buffer;
    }
    int64_t new_cap = *cap * 2 + needed + 64;
    char* bigger = (char*)sw_gc_alloc((uint64_t)new_cap);
    if (used > 0) {
        memcpy(bigger, buffer, (uint64_t)used);
    }
    *cap = new_cap;
    return bigger;
}

sw_string* format(sw_string* fmt, sw_array* args) {
    if (fmt == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t arg_count = args != NULL ? args->len : 0;
    int64_t next_arg = 0;
    int64_t cap = fmt->len * 2 + 64;
    char* buffer = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t used = 0;
    int64_t i = 0;
    while (i < fmt->len) {
        if (fmt->data[i] != '%') {
            buffer = sw_format_grow(buffer, &cap, used, 1);
            buffer[used++] = fmt->data[i++];
            continue;
        }
        int64_t spec_start = i;
        i++;
        if (i < fmt->len && fmt->data[i] == '%') {
            buffer = sw_format_grow(buffer, &cap, used, 1);
            buffer[used++] = '%';
            i++;
            continue;
        }
        while (i < fmt->len && !sw_format_conv(fmt->data[i])) {
            i++;
        }
        if (i >= fmt->len) {
            buffer = sw_format_grow(buffer, &cap, used, 1);
            buffer[used++] = '%';
            break;
        }
        char conv = fmt->data[i];
        int is_int = conv == 'd' || conv == 'i' || conv == 'u' ||
                     conv == 'x' || conv == 'X' || conv == 'o' || conv == 'c';
        char spec[64];
        int64_t spec_len = 0;
        spec[spec_len++] = '%';
        for (int64_t k = spec_start + 1; k < i; k++) {
            if (spec_len < (int64_t)sizeof(spec) - 4) {
                spec[spec_len++] = fmt->data[k];
            }
        }
        if (is_int && conv != 'c') {
            spec[spec_len++] = 'l';
            spec[spec_len++] = 'l';
        }
        spec[spec_len++] = conv;
        spec[spec_len] = 0;

        int64_t tag = SW_TAG_INT;
        int64_t value = 0;
        if (next_arg * 2 + 1 < arg_count * 2) {
            tag = ((int64_t*)args->data)[next_arg * 2];
            value = ((int64_t*)args->data)[next_arg * 2 + 1];
        }
        next_arg++;

        int64_t needed = 0;
        if (conv == 's') {
            sw_string* text = (sw_string*)value;
            const char* data = text != NULL ? text->data : "(null)";
            (void)tag;
            needed = snprintf(NULL, 0, spec, data);
        } else if (conv == 'c') {
            needed = snprintf(NULL, 0, spec, (int)(value & 0xFF));
        } else if (is_int) {
            needed = snprintf(NULL, 0, spec, (long long)value);
        } else {
            double d;
            memcpy(&d, &value, 8);
            needed = snprintf(NULL, 0, spec, d);
        }
        if (needed < 0) {
            needed = 0;
        }
        buffer = sw_format_grow(buffer, &cap, used, needed);
        if (conv == 's') {
            sw_string* text = (sw_string*)value;
            const char* data = text != NULL ? text->data : "(null)";
            int64_t written = snprintf(buffer + used, (sw_size)(cap - used), spec, data);
            used += written > 0 ? written : 0;
        } else if (conv == 'c') {
            int64_t written =
                snprintf(buffer + used, (sw_size)(cap - used), spec, (int)(value & 0xFF));
            used += written > 0 ? written : 0;
        } else if (is_int) {
            int64_t written =
                snprintf(buffer + used, (sw_size)(cap - used), spec, (long long)value);
            used += written > 0 ? written : 0;
        } else {
            double d;
            memcpy(&d, &value, 8);
            int64_t written = snprintf(buffer + used, (sw_size)(cap - used), spec, d);
            used += written > 0 ? written : 0;
        }
        i++;
    }
    return sw_string_from_literal(buffer, used);
}

// print_format(fmt, ...args)：按 format 格式化后直接输出（不换行）。
void sw_print_format(sw_string* fmt, sw_array* args) {
    sw_string* text = format(fmt, args);
    sw_print_string(text);
}

int64_t parse_date(sw_string* text) {
    int64_t index = 0;
    while (index < text->len &&
           (text->data[index] == ' ' || text->data[index] == '\t')) {
        index++;
    }
    int year = 0;
    int month = 0;
    int day = 0;
    for (int digit = 0; digit < 4; digit++) {
        if (index >= text->len || text->data[index] < '0' || text->data[index] > '9') {
            return -1;
        }
        year = year * 10 + (text->data[index] - '0');
        index++;
    }
    if (index >= text->len || text->data[index] != '-') {
        return -1;
    }
    index++;
    for (int digit = 0; digit < 2; digit++) {
        if (index >= text->len || text->data[index] < '0' || text->data[index] > '9') {
            return -1;
        }
        month = month * 10 + (text->data[index] - '0');
        index++;
    }
    if (index >= text->len || text->data[index] != '-') {
        return -1;
    }
    index++;
    for (int digit = 0; digit < 2; digit++) {
        if (index >= text->len || text->data[index] < '0' || text->data[index] > '9') {
            return -1;
        }
        day = day * 10 + (text->data[index] - '0');
        index++;
    }
    while (index < text->len &&
           (text->data[index] == ' ' || text->data[index] == '\t')) {
        index++;
    }
    if (index != text->len || month < 1 || month > 12 || day < 1 || day > 31) {
        return -1;
    }
#if defined(_WIN32)
    extern int TzSpecificLocalTimeToSystemTime(const void* time_zone, const void* local_time, void* utc_time);
    extern int SystemTimeToFileTime(const void* system_time, void* file_time);
    unsigned char st[16] = {0};
    unsigned char utc_st[16] = {0};
    *(unsigned short*)st = (unsigned short)year;
    *(unsigned short*)(st + 2) = (unsigned short)month;
    *(unsigned short*)(st + 6) = (unsigned short)day;
    if (!TzSpecificLocalTimeToSystemTime(NULL, st, utc_st)) {
        return -1;
    }
    unsigned char ft[8];
    if (!SystemTimeToFileTime(utc_st, ft)) {
        return -1;
    }
    uint64_t since_1601 =
        ((uint64_t)(*(unsigned int*)(ft + 4)) << 32) | (*(unsigned int*)ft);
    return (int64_t)(since_1601 / 10000000 - 11644473600ULL);
#else
    extern long mktime(void* tm);
    unsigned char tm[64] = {0};
    *(int*)(tm + 0) = 0;  // tm_sec
    *(int*)(tm + 4) = 0;  // tm_min
    *(int*)(tm + 8) = 0;  // tm_hour
    *(int*)(tm + 12) = day;
    *(int*)(tm + 16) = month - 1;
    *(int*)(tm + 20) = year - 1900;
    *(int*)(tm + 32) = -1;  // tm_isdst
    long result = mktime(tm);
    return result == (long)-1 ? -1 : (int64_t)result;
#endif
}

void sleep_ms(int64_t milliseconds) {
    if (milliseconds <= 0) {
        return;
    }
#if defined(_WIN32)
    Sleep((unsigned long)milliseconds);
#else
    unsigned char req[16];
    *(int64_t*)req = milliseconds / 1000;
    *(int64_t*)(req + 8) = (milliseconds % 1000) * 1000000;
    nanosleep(req, NULL);
#endif
}

// ---------------------------------------------------------------------------
// json：最小 JSON 解析器（GC 分配的标记值）。
// ---------------------------------------------------------------------------

typedef struct sw_json {
    int64_t kind; // 0 null 1 bool 2 int 3 float 4 string 5 array 6 object
    int64_t int_value;
    double float_value;
    char* string_value;
    struct sw_json** items;
    char** keys;
    int64_t length;
} sw_json;

static int sw_json_skip_space(const char* text, int64_t len, int64_t* pos);
static sw_json* sw_json_parse_value(const char* text, int64_t len, int64_t* pos);

static int sw_json_skip_space(const char* text, int64_t len, int64_t* pos) {
    while (*pos < len && (text[*pos] == ' ' || text[*pos] == '\t' || text[*pos] == '\n' ||
                          text[*pos] == '\r')) {
        (*pos)++;
    }
    return *pos < len;
}

static sw_json* sw_json_make(int64_t kind) {
    sw_json* value = (sw_json*)sw_gc_alloc(sizeof(sw_json));
    value->kind = kind;
    value->int_value = 0;
    value->float_value = 0;
    value->string_value = NULL;
    value->items = NULL;
    value->keys = NULL;
    value->length = 0;
    return value;
}

static sw_json* sw_json_parse_string(const char* text, int64_t len, int64_t* pos) {
    // text[*pos] == '"'
    int64_t start = *pos + 1;
    int64_t end = start;
    while (end < len && text[end] != '"') {
        if (text[end] == '\\') {
            end++;
        }
        end++;
    }
    if (end >= len) {
        return NULL;
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)(end - start) + 1);
    int64_t out = 0;
    for (int64_t i = start; i < end; i++) {
        if (text[i] == '\\' && i + 1 < end) {
            char next = text[i + 1];
            switch (next) {
                case 'n': buffer[out++] = '\n'; break;
                case 't': buffer[out++] = '\t'; break;
                case 'r': buffer[out++] = '\r'; break;
                case '\\': buffer[out++] = '\\'; break;
                case '"': buffer[out++] = '"'; break;
                case '/': buffer[out++] = '/'; break;
                default: buffer[out++] = next; break;
            }
            i++;
        } else {
            buffer[out++] = text[i];
        }
    }
    buffer[out] = 0;
    *pos = end + 1;
    sw_json* value = sw_json_make(4);
    value->string_value = buffer;
    value->length = out;
    return value;
}

static sw_json* sw_json_parse_value(const char* text, int64_t len, int64_t* pos) {
    if (!sw_json_skip_space(text, len, pos)) {
        return NULL;
    }
    char c = text[*pos];
    if (c == '"') {
        return sw_json_parse_string(text, len, pos);
    }
    if (c == '{') {
        (*pos)++;
        sw_json* object = sw_json_make(6);
        int64_t capacity = 4;
        object->items = (sw_json**)sw_gc_alloc((uint64_t)capacity * sizeof(sw_json*));
        object->keys = (char**)sw_gc_alloc((uint64_t)capacity * sizeof(char*));
        while (sw_json_skip_space(text, len, pos) && text[*pos] != '}') {
            if (text[*pos] != '"') {
                return NULL;
            }
            sw_json* key = sw_json_parse_string(text, len, pos);
            if (key == NULL || !sw_json_skip_space(text, len, pos) || text[*pos] != ':') {
                return NULL;
            }
            (*pos)++;
            sw_json* value = sw_json_parse_value(text, len, pos);
            if (value == NULL) {
                return NULL;
            }
            if (object->length >= capacity) {
                int64_t old_capacity = capacity;
                capacity *= 2;
                sw_json** new_items =
                    (sw_json**)sw_gc_alloc((uint64_t)capacity * sizeof(sw_json*));
                char** new_keys = (char**)sw_gc_alloc((uint64_t)capacity * sizeof(char*));
                memcpy(new_items, object->items, (sw_size)(old_capacity * sizeof(sw_json*)));
                memcpy(new_keys, object->keys, (sw_size)(old_capacity * sizeof(char*)));
                object->items = new_items;
                object->keys = new_keys;
            }
            object->keys[object->length] = key->string_value;
            object->items[object->length] = value;
            object->length++;
            if (!sw_json_skip_space(text, len, pos)) {
                return NULL;
            }
            if (text[*pos] == ',') {
                (*pos)++;
            } else if (text[*pos] != '}') {
                return NULL;
            }
        }
        if (*pos >= len || text[*pos] != '}') {
            return NULL;
        }
        (*pos)++;
        return object;
    }
    if (c == '[') {
        (*pos)++;
        sw_json* array = sw_json_make(5);
        int64_t capacity = 4;
        array->items = (sw_json**)sw_gc_alloc((uint64_t)capacity * sizeof(sw_json*));
        while (sw_json_skip_space(text, len, pos) && text[*pos] != ']') {
            sw_json* value = sw_json_parse_value(text, len, pos);
            if (value == NULL) {
                return NULL;
            }
            if (array->length >= capacity) {
                int64_t old_capacity = capacity;
                capacity *= 2;
                sw_json** new_items =
                    (sw_json**)sw_gc_alloc((uint64_t)capacity * sizeof(sw_json*));
                memcpy(new_items, array->items, (sw_size)(old_capacity * sizeof(sw_json*)));
                array->items = new_items;
            }
            array->items[array->length++] = value;
            if (!sw_json_skip_space(text, len, pos)) {
                return NULL;
            }
            if (text[*pos] == ',') {
                (*pos)++;
            } else if (text[*pos] != ']') {
                return NULL;
            }
        }
        if (*pos >= len || text[*pos] != ']') {
            return NULL;
        }
        (*pos)++;
        return array;
    }
    if (c == 't' && *pos + 4 <= len) {
        if (text[*pos + 1] == 'r' && text[*pos + 2] == 'u' && text[*pos + 3] == 'e') {
            *pos += 4;
            sw_json* value = sw_json_make(1);
            value->int_value = 1;
            return value;
        }
    }
    if (c == 'f' && *pos + 5 <= len) {
        if (text[*pos + 1] == 'a' && text[*pos + 2] == 'l' && text[*pos + 3] == 's' &&
            text[*pos + 4] == 'e') {
            *pos += 5;
            sw_json* value = sw_json_make(1);
            value->int_value = 0;
            return value;
        }
    }
    if (c == 'n' && *pos + 4 <= len) {
        if (text[*pos + 1] == 'u' && text[*pos + 2] == 'l' && text[*pos + 3] == 'l') {
            *pos += 4;
            return sw_json_make(0);
        }
    }
    // 数字
    int64_t start = *pos;
    int64_t is_float = 0;
    while (*pos < len) {
        char digit = text[*pos];
        if (digit == '.' || digit == 'e' || digit == 'E' || digit == '+' || digit == '-') {
            is_float = 1;
        } else if (!(digit >= '0' && digit <= '9')) {
            break;
        }
        (*pos)++;
    }
    if (*pos == start) {
        return NULL;
    }
    char* number = (char*)sw_gc_alloc((uint64_t)(*pos - start) + 1);
    for (int64_t i = start; i < *pos; i++) {
        number[i - start] = text[i];
    }
    number[*pos - start] = 0;
    sw_json* value = sw_json_make(is_float ? 3 : 2);
    if (is_float) {
        extern double atof(const char* text);
        value->float_value = atof(number);
    } else {
        int64_t parsed = 0;
        int64_t negative = 0;
        int64_t i = 0;
        if (number[0] == '-') {
            negative = 1;
            i = 1;
        }
        while (number[i] != 0) {
            parsed = parsed * 10 + (number[i] - '0');
            i++;
        }
        value->int_value = negative ? -parsed : parsed;
    }
    return value;
}

void* json_parse(sw_string* text) {
    int64_t pos = 0;
    return (void*)sw_json_parse_value(text->data, text->len, &pos);
}

int64_t json_kind(void* value) {
    return value == NULL ? 0 : ((sw_json*)value)->kind;
}

int64_t json_bool(void* value) {
    return value == NULL ? 0 : ((sw_json*)value)->int_value;
}

int64_t json_int(void* value) {
    return value == NULL ? 0 : ((sw_json*)value)->int_value;
}

double json_float(void* value) {
    return value == NULL ? 0.0 : ((sw_json*)value)->float_value;
}

sw_string* json_string(void* value) {
    if (value == NULL || ((sw_json*)value)->string_value == NULL) {
        return sw_string_from_literal("", 0);
    }
    return sw_string_from_literal(((sw_json*)value)->string_value, ((sw_json*)value)->length);
}

int64_t json_array_len(void* value) {
    return value == NULL ? 0 : ((sw_json*)value)->length;
}

void* json_array_at(void* value, int64_t index) {
    sw_json* array = (sw_json*)value;
    if (array == NULL || array->kind != 5 || index < 0 || index >= array->length) {
        return NULL;
    }
    return array->items[index];
}

void* json_object_get(void* value, sw_string* key) {
    sw_json* object = (sw_json*)value;
    if (object == NULL || object->kind != 6) {
        return NULL;
    }
    // 按键名线性查找（长度通过临时字符串计算）。
    for (int64_t index = 0; index < object->length; index++) {
        int64_t key_len = 0;
        while (object->keys[index][key_len] != 0) {
            key_len++;
        }
        int64_t match = key_len == key->len ? 1 : 0;
        if (match) {
            for (int64_t i = 0; i < key_len; i++) {
                if (object->keys[index][i] != key->data[i]) {
                    match = 0;
                    break;
                }
            }
        }
        if (match) {
            return object->items[index];
        }
    }
    return NULL;
}

sw_array* sw_array_new(int64_t elem_size, int64_t count) {
    if (count < 0) {
        count = 0;
    }
    if (elem_size < 1) {
        elem_size = 1;
    }
    sw_array* array =
        (sw_array*)sw_gc_alloc(sizeof(sw_array) + (uint64_t)count * (uint64_t)elem_size);
    array->len = count;
    array->cap = count;
    array->data = (void*)(array + 1);
    memset(array->data, 0, (sw_size)((uint64_t)count * (uint64_t)elem_size));
    return array;
}

void sw_array_set(sw_array* array, int64_t index, int64_t value) {
    ((int64_t*)array->data)[index] = value;
}

// 追加元素（扩容：新数组 + 复制），返回新长度。
int64_t sw_array_push(sw_array* array, int64_t value) {
    if (array == NULL) {
        return 0;
    }
    if (array->len >= array->cap) {
        int64_t new_cap = array->cap * 2 + 1;
        sw_array* bigger = sw_array_new(8, new_cap);
        memcpy(bigger->data, array->data, (sw_size)((uint64_t)array->len * 8));
        bigger->len = array->len;
        *array = *bigger;
    }
    ((int64_t*)array->data)[array->len] = value;
    array->len += 1;
    return array->len;
}

// 弹出末尾元素（空数组返回 0）。
int64_t sw_array_pop(sw_array* array) {
    if (array == NULL || array->len <= 0) {
        return 0;
    }
    array->len -= 1;
    return ((int64_t*)array->data)[array->len];
}

// 数组切片（复制）：a[start:end]，越界自动裁剪，返回新数组。
sw_array* sw_array_slice(sw_array* array, int64_t start, int64_t end, int64_t elem_size) {
    if (elem_size <= 0) {
        elem_size = 1;
    }
    if (array == NULL) {
        return sw_array_new(elem_size, 0);
    }
    if (start < 0) {
        start = 0;
    }
    if (end > array->len) {
        end = array->len;
    }
    if (start >= end) {
        return sw_array_new(elem_size, 0);
    }
    int64_t count = end - start;
    sw_array* result = sw_array_new(elem_size, count);
    memcpy(
        result->data,
        (char*)array->data + (uintptr_t)start * elem_size,
        (sw_size)((uint64_t)count * elem_size)
    );
    return result;
}

void sw_array_set_u8(sw_array* array, int64_t index, int64_t value) {
    ((unsigned char*)array->data)[index] = (unsigned char)value;
}

sw_array* read_file_bytes(sw_string* path) {
    sw_file_handle* file = fopen(path->data, "rb");
    if (file == NULL) {
        return sw_array_new(1, 0);
    }
    fseek(file, 0, 2);
    long size = ftell(file);
    rewind(file);
    if (size < 0) {
        size = 0;
    }
    sw_array* array = sw_array_new(1, size);
    if (size > 0) {
        fread(array->data, 1, (uint64_t)size, file);
    }
    fclose(file);
    return array;
}

int64_t write_file_bytes(sw_string* path, sw_array* bytes) {
    sw_file_handle* file = fopen(path->data, "wb");
    if (file == NULL) {
        return -1;
    }
    int64_t written = (int64_t)fwrite(bytes->data, 1, (uint64_t)bytes->len, file);
    fclose(file);
    return written;
}

void* sw_object_new(int64_t size) {
    if (size < 0) {
        size = 0;
    }
    void* object = sw_gc_alloc((uint64_t)size);
    memset(object, 0, (sw_size)(uint64_t)size);
    return object;
}

// ---------------------------------------------------------------------------
// 闭包：{ fn 指针, 环境槽数组 }
// ---------------------------------------------------------------------------

typedef struct {
    void* fn;
    void* env;
} sw_closure;

void* sw_closure_new(void* fn, int64_t env_slots) {
    if (env_slots < 0) {
        env_slots = 0;
    }
    sw_closure* closure =
        (sw_closure*)sw_gc_alloc(sizeof(sw_closure) + (uint64_t)env_slots * 8);
    closure->fn = fn;
    closure->env = (void**)(closure + 1);
    memset(closure->env, 0, (sw_size)((uint64_t)env_slots * 8));
    return closure;
}

void sw_env_set(void* closure, int64_t slot, int64_t value) {
    ((int64_t*)(((sw_closure*)closure)->env))[slot] = value;
}

int64_t sw_env_get(void* closure, int64_t slot) {
    return ((int64_t*)(((sw_closure*)closure)->env))[slot];
}

// `**` 幂运算：整数用快速幂循环（负指数返回 0），浮点走 libm pow。
int64_t sw_pow_i64(int64_t base, int64_t exp) {
    if (exp < 0) {
        return 0;
    }
    int64_t result = 1;
    while (exp > 0) {
        if (exp & 1) {
            result *= base;
        }
        base *= base;
        exp >>= 1;
    }
    return result;
}

extern double pow(double base, double exp);

double sw_pow_f64(double base, double exp) {
    return pow(base, exp);
}

extern double fmod(double numerator, double denominator);

double sw_frem_f64(double numerator, double denominator) {
    return fmod(numerator, denominator);
}

// ---------------------------------------------------------------------------
// 异常：setjmp/longjmp 传播（不展开平台栈）
// ---------------------------------------------------------------------------

typedef struct sw_exception {
    int64_t type_id;
    void* value;
} sw_exception;

typedef struct sw_frame {
    unsigned char buf[0x140];
    sw_exception* exception;
    struct sw_frame* prev;
} sw_frame;

// v0.1 单线程：异常框架使用普通全局；多线程支持留到后续版本。
static sw_frame* sw_current_frame = NULL;


void* sw_try_begin(void) {
    // 帧由 GC 管理：sw_current_frame 是全局根（数据段被 GC 扫描），
    // try_leave 出链后自然可回收，不再 malloc/free。
    sw_frame* frame = (sw_frame*)sw_gc_alloc(sizeof(sw_frame));
    frame->exception = NULL;
    frame->prev = sw_current_frame;
    sw_current_frame = frame;
    return frame;
}

void* sw_try_value(void* frame) {
    return ((sw_frame*)frame)->exception;
}

void sw_try_leave(void* frame) {
    sw_frame* current = (sw_frame*)frame;
    sw_current_frame = current->prev;
}

void sw_throw(void* value, int64_t type_id) {
    sw_frame* frame = sw_current_frame;
    // 异常对象由 GC 管理：挂在帧上（帧在链上直到 try_leave），catch 后随帧一起回收。
    sw_exception* exception = (sw_exception*)sw_gc_alloc(sizeof(sw_exception));
    exception->type_id = type_id;
    exception->value = value;
    frame->exception = exception;
    sw_longjmp(frame->buf, 1);
}

void sw_rethrow(void* exception) {
    sw_frame* frame = sw_current_frame;
    frame->exception = (sw_exception*)exception;
    sw_longjmp(frame->buf, 1);
}

int64_t sw_exception_type(void* exception) {
    return ((sw_exception*)exception)->type_id;
}

void* sw_exception_value(void* exception) {
    return ((sw_exception*)exception)->value;
}

// aarch64 Linux 静态链接 musl 时需要 compiler-rt 的 128 位软浮点辅助函数；
// 这些函数来自 rustup 的 libcompiler_builtins.rlib，其对象引用了
// rust_eh_personality（本运行时无 Rust 异常，桩实现即可满足链接）。
#if defined(__aarch64__) && !defined(_WIN32)
void rust_eh_personality(void) {}
#endif

// ---------------------------------------------------------------------------
// 进程入口：把命令行参数构造成 string[]，调用用户 main(args)。
// 用户 main 可声明为 `function main(): int` 或 `function main(args: string[]): int`。
// ---------------------------------------------------------------------------

extern int64_t sw_user_main(void* args);

#if defined(_WIN32)
// MinGW 目标下 clang 在 main 开头调用 __main（GCC 风格 CRT 初始化钩子）；
// 我们用自己的 startup，无需 CRT 初始化，空实现即可。
void __main(void) {}
#else
extern char** environ;
#endif

// 前置声明（定义在文件后部，run_with_env 等需要）。
sw_string* os_which(sw_string* name);

#if !defined(SW_NO_MAIN)
int main(int argc, char** argv) {
#if defined(_WIN32)
    // 控制台代码页切到 UTF-8：否则中文输出在 GBK 控制台显示乱码。
    extern int SetConsoleOutputCP(unsigned int cp);
    extern int SetConsoleCP(unsigned int cp);
    SetConsoleOutputCP(65001);
    SetConsoleCP(65001);
    // 启用 VT 转义序列处理（ANSI 颜色/清屏/光标；std/console 依赖）。
    sw_enable_vt();
    // ucrt 不导出 __argc/__argv，用 CommandLineToArgvW + UTF-8 转换。
    extern void* GetCommandLineW(void);
    extern void** CommandLineToArgvW(void* cmdline, int* out_argc);
    extern void LocalFree(void* ptr);
    extern int WideCharToMultiByte(
        unsigned int cp,
        unsigned long flags,
        const void* wstr,
        int wlen,
        char* out,
        int outlen,
        const void* default_char,
        const void* used_default
    );
    void* cmdline = GetCommandLineW();
    void** wide_argv = CommandLineToArgvW(cmdline, &argc);
    char** converted = (char**)sw_gc_alloc((uint64_t)argc * sizeof(char*));
    for (int64_t index = 0; index < argc; index++) {
        int len = WideCharToMultiByte(65001, 0, wide_argv[index], -1, NULL, 0, NULL, NULL);
        if (len <= 0) {
            len = 1;
        }
        char* copy = (char*)sw_gc_alloc((uint64_t)len);
        WideCharToMultiByte(65001, 0, wide_argv[index], -1, copy, len, NULL, NULL);
        converted[index] = copy;
    }
    LocalFree(wide_argv);
    argv = converted;
#else
    (void)argc;
    (void)argv;
#endif
    sw_array* args_array = sw_array_new(8, argc);
    for (int64_t index = 0; index < argc; index++) {
        const char* arg = argv[index];
        ((int64_t*)args_array->data)[index] =
            (int64_t)sw_string_from_literal(arg, (int64_t)strlen(arg));
    }
    int64_t result = sw_user_main(args_array);
    exit((int)result);
    return 0;
}
#elif defined(_WIN32)
// DLL 模式：提供 CRT 约定的入口（无初始化，返回成功）。
int DllMainCRTStartup(void* instance, unsigned long reason, void* reserved) {
    (void)instance;
    (void)reason;
    (void)reserved;
    return 1;
}
#endif

sw_string* sw_getenv(sw_string* name) {
#if defined(_WIN32)
    extern int GetEnvironmentVariableA(const char* name, char* buffer, unsigned int size);
    char buffer[4096];
    unsigned int size = GetEnvironmentVariableA(name->data, buffer, sizeof(buffer));
    if (size == 0) {
        return NULL;
    }
    return sw_string_from_literal(buffer, (int64_t)size);
#else
    for (int64_t index = 0; environ[index] != NULL; index++) {
        const char* entry = environ[index];
        int64_t name_len = name->len;
        int64_t entry_len = 0;
        while (entry[entry_len] != 0 && entry[entry_len] != '=') {
            entry_len++;
        }
        if (entry_len == name_len) {
            int64_t match = 1;
            for (int64_t i = 0; i < name_len; i++) {
                if (entry[i] != name->data[i]) {
                    match = 0;
                    break;
                }
            }
            if (match) {
                return sw_string_from_literal(
                    entry + name_len + 1,
                    (int64_t)strlen(entry + name_len + 1)
                );
            }
        }
    }
    return NULL;
#endif
}

// ---------------------------------------------------------------------------
// 进程：spawn / wait / poll / kill / run / run_with_input / run_status
// Windows 用 CreateProcessA + 管道；POSIX 用 fork + execvp + 管道。
// 注意：Sw 侧名字经 codegen 映射为 sw_ 前缀（避免遮蔽 libc 同名符号）。
// ---------------------------------------------------------------------------

#define SW_MAX_PROCESSES 64

typedef struct sw_proc_entry {
    int64_t pid;
    void* handle;
    int64_t used;
} sw_proc_entry;

static sw_proc_entry sw_processes[SW_MAX_PROCESSES];

static void* sw_proc_handle(int64_t pid) {
    for (int64_t i = 0; i < SW_MAX_PROCESSES; i++) {
        if (sw_processes[i].used && sw_processes[i].pid == pid) {
            return sw_processes[i].handle;
        }
    }
    return NULL;
}

static void sw_proc_store(int64_t pid, void* handle) {
    for (int64_t i = 0; i < SW_MAX_PROCESSES; i++) {
        if (!sw_processes[i].used) {
            sw_processes[i].used = 1;
            sw_processes[i].pid = pid;
            sw_processes[i].handle = handle;
            return;
        }
    }
}

static void sw_proc_clear(int64_t pid) {
    for (int64_t i = 0; i < SW_MAX_PROCESSES; i++) {
        if (sw_processes[i].used && sw_processes[i].pid == pid) {
            sw_processes[i].used = 0;
            return;
        }
    }
}

// argv 数组：argv[0]=cmd，其后为 args，末尾 NULL（POSIX execvp 需要）。
static char** sw_build_argv(sw_string* cmd, sw_array* args) {
    int64_t count = 1 + args->len;
    char** argv = (char**)sw_gc_alloc((uint64_t)(count + 1) * sizeof(char*));
    argv[0] = cmd->data;
    for (int64_t i = 0; i < args->len; i++) {
        sw_string* item = (sw_string*)((int64_t*)args->data)[i];
        argv[i + 1] = item->data;
    }
    argv[count] = NULL;
    return argv;
}

#if defined(_WIN32)

// 把 argv 拼成 CreateProcessA 的命令行：含空格/制表/引号的参数用双引号包裹，内部引号双写。
static char* sw_build_cmdline(sw_string* cmd, sw_array* args) {
    int64_t total = cmd->len + 4;
    for (int64_t i = 0; i < args->len; i++) {
        sw_string* item = (sw_string*)((int64_t*)args->data)[i];
        int64_t extra = 0;
        for (int64_t j = 0; j < item->len; j++) {
            if (item->data[j] == '"') {
                extra++;
            }
        }
        total += item->len + extra + 4;
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)total + 1);
    int64_t out = 0;
    for (int64_t i = 0; i < args->len + 1; i++) {
        sw_string* part =
            i == 0 ? cmd : (sw_string*)((int64_t*)args->data)[i - 1];
        int64_t need_quote = part->len == 0;
        for (int64_t j = 0; j < part->len && !need_quote; j++) {
            char ch = part->data[j];
            if (ch == ' ' || ch == '\t' || ch == '"') {
                need_quote = 1;
            }
        }
        if (i > 0) {
            buffer[out++] = ' ';
        }
        if (need_quote) {
            buffer[out++] = '"';
        }
        for (int64_t j = 0; j < part->len; j++) {
            char ch = part->data[j];
            if (ch == '"') {
                buffer[out++] = '"';
            }
            buffer[out++] = ch;
        }
        if (need_quote) {
            buffer[out++] = '"';
        }
    }
    buffer[out] = 0;
    return buffer;
}

// STARTUPINFOA（x64 104 字节）与 PROCESS_INFORMATION（24 字节）。
typedef struct sw_startup_info {
    unsigned int cb;
    char* reserved;
    char* desktop;
    char* title;
    unsigned int x;
    unsigned int y;
    unsigned int x_size;
    unsigned int y_size;
    unsigned int x_chars;
    unsigned int y_chars;
    unsigned int fill;
    unsigned int flags;
    unsigned short show_window;
    unsigned short reserved2_count;
    unsigned char* reserved2;
    void* h_std_input;
    void* h_std_output;
    void* h_std_error;
} sw_startup_info;

typedef struct sw_proc_info {
    void* h_process;
    void* h_thread;
    unsigned int process_id;
    unsigned int thread_id;
} sw_proc_info;

extern int CreatePipe(void** read_handle, void** write_handle, void* attr, unsigned int size);
extern int SetHandleInformation(void* handle, unsigned int mask, unsigned int flags);
extern int CreateProcessA(
    const char* app,
    char* cmdline,
    void* proc_attr,
    void* thread_attr,
    int inherit,
    unsigned int flags,
    void* env,
    const char* cwd,
    void* startup,
    void* proc_info
);
extern unsigned long WaitForSingleObject(void* handle, unsigned long ms);
extern int GetExitCodeProcess(void* handle, unsigned int* code);
extern int TerminateProcess(void* handle, unsigned int code);
extern int CloseHandle(void* handle);
extern int ReadFile(void* handle, void* buffer, unsigned int bytes, unsigned int* read, void* overlapped);
extern int WriteFile(void* handle, const void* buffer, unsigned int bytes, unsigned int* written, void* overlapped);

int64_t sw_spawn(sw_string* cmd, sw_array* args) {
    char* cmdline = sw_build_cmdline(cmd, args);
    sw_startup_info startup;
    memset(&startup, 0, sizeof(startup));
    startup.cb = sizeof(startup);
    sw_proc_info info;
    memset(&info, 0, sizeof(info));
    if (!CreateProcessA(NULL, cmdline, NULL, NULL, 1, 0, NULL, NULL, &startup, &info)) {
        return 0;
    }
    int64_t pid = (int64_t)info.process_id;
    CloseHandle(info.h_thread);
    sw_proc_store(pid, info.h_process);
    return pid;
}

int64_t sw_wait(int64_t pid) {
    void* handle = sw_proc_handle(pid);
    if (handle == NULL) {
        return -1;
    }
    WaitForSingleObject(handle, 0xFFFFFFFFu);
    unsigned int code = 0;
    GetExitCodeProcess(handle, &code);
    CloseHandle(handle);
    sw_proc_clear(pid);
    return (int64_t)code;
}

int64_t sw_poll(int64_t pid) {
    void* handle = sw_proc_handle(pid);
    if (handle == NULL) {
        return -2;
    }
    if (WaitForSingleObject(handle, 0) == 0x00000102u) {  // WAIT_TIMEOUT
        return -1;
    }
    unsigned int code = 0;
    GetExitCodeProcess(handle, &code);
    CloseHandle(handle);
    sw_proc_clear(pid);
    return (int64_t)code;
}

int64_t sw_kill(int64_t pid) {
    void* handle = sw_proc_handle(pid);
    if (handle == NULL) {
        return -1;
    }
    return TerminateProcess(handle, 1) ? 0 : -1;
}

static sw_string* sw_run_impl(sw_string* cmd, sw_array* args, sw_string* input) {
    void* out_read = NULL;
    void* out_write = NULL;
    void* in_read = NULL;
    void* in_write = NULL;
    int64_t has_input = input != NULL && input->len > 0;
    if (!CreatePipe(&out_read, &out_write, NULL, 0)) {
        return sw_string_from_literal("", 0);
    }
    SetHandleInformation(out_write, 1, 1);
    if (has_input) {
        if (!CreatePipe(&in_read, &in_write, NULL, 0)) {
            CloseHandle(out_read);
            CloseHandle(out_write);
            return sw_string_from_literal("", 0);
        }
        SetHandleInformation(in_read, 1, 1);
    }
    char* cmdline = sw_build_cmdline(cmd, args);
    sw_startup_info startup;
    memset(&startup, 0, sizeof(startup));
    startup.cb = sizeof(startup);
    startup.flags = 0x00000100u;  // STARTF_USESTDHANDLES
    startup.h_std_output = out_write;
    startup.h_std_error = out_write;
    startup.h_std_input = in_read;
    sw_proc_info info;
    memset(&info, 0, sizeof(info));
    int ok = CreateProcessA(NULL, cmdline, NULL, NULL, 1, 0, NULL, NULL, &startup, &info);
    CloseHandle(out_write);
    if (in_read != NULL) {
        CloseHandle(in_read);
    }
    if (!ok) {
        CloseHandle(out_read);
        if (in_write != NULL) {
            CloseHandle(in_write);
        }
        return sw_string_from_literal("", 0);
    }
    CloseHandle(info.h_thread);
    if (has_input) {
        unsigned int written = 0;
        WriteFile(in_write, input->data, (unsigned int)input->len, &written, NULL);
    }
    if (in_write != NULL) {
        CloseHandle(in_write);
    }
    char chunk[4096];
    char* buffer = (char*)malloc(4096);
    int64_t capacity = 4096;
    int64_t length = 0;
    while (1) {
        unsigned int got = 0;
        if (!ReadFile(out_read, chunk, sizeof(chunk), &got, NULL) || got == 0) {
            break;
        }
        if (length + (int64_t)got > capacity) {
            capacity = (length + (int64_t)got) * 2;
            buffer = (char*)realloc(buffer, (sw_size)capacity);
        }
        memcpy(buffer + length, chunk, got);
        length += (int64_t)got;
    }
    CloseHandle(out_read);
    WaitForSingleObject(info.h_process, 0xFFFFFFFFu);
    CloseHandle(info.h_process);
    sw_string* result = sw_string_from_literal(buffer, length);
    free(buffer);
    return result;
}

int64_t sw_run_status(sw_string* cmd, sw_array* args) {
    char* cmdline = sw_build_cmdline(cmd, args);
    sw_startup_info startup;
    memset(&startup, 0, sizeof(startup));
    startup.cb = sizeof(startup);
    sw_proc_info info;
    memset(&info, 0, sizeof(info));
    if (!CreateProcessA(NULL, cmdline, NULL, NULL, 1, 0, NULL, NULL, &startup, &info)) {
        return -1;
    }
    CloseHandle(info.h_thread);
    WaitForSingleObject(info.h_process, 0xFFFFFFFFu);
    unsigned int code = 0;
    GetExitCodeProcess(info.h_process, &code);
    CloseHandle(info.h_process);
    return (int64_t)code;
}

#else  // POSIX（Linux / macOS）

extern int pipe(int fds[2]);
extern int fork(void);
extern int execvp(const char* file, char* const argv[]);
extern int waitpid(int pid, int* status, int options);
extern int kill(int pid, int signal);
extern long read(int fd, void* buffer, unsigned long count);
extern long write(int fd, const void* buffer, unsigned long count);
extern int close(int fd);
extern int dup2(int old_fd, int new_fd);
extern void _exit(int code);

int64_t sw_spawn(sw_string* cmd, sw_array* args) {
    char** argv = sw_build_argv(cmd, args);
    int pid = fork();
    if (pid < 0) {
        return 0;
    }
    if (pid == 0) {
        execvp(argv[0], argv);
        _exit(127);
    }
    return (int64_t)pid;
}

int64_t sw_wait(int64_t pid) {
    int status = 0;
    if (waitpid((int)pid, &status, 0) < 0) {
        return -1;
    }
    int code = status & 0x7f;
    if (code == 0) {
        return (status >> 8) & 0xff;
    }
    return 128 + code;  // 被信号杀死：128+信号号
}

int64_t sw_poll(int64_t pid) {
    int status = 0;
    int result = waitpid((int)pid, &status, 1);  // WNOHANG=1
    if (result == 0) {
        return -1;  // 仍在运行
    }
    if (result < 0) {
        return -2;  // 未知或已回收
    }
    int code = status & 0x7f;
    if (code == 0) {
        return (status >> 8) & 0xff;
    }
    return 128 + code;
}

int64_t sw_kill(int64_t pid) {
    return kill((int)pid, 9) == 0 ? 0 : -1;  // SIGKILL=9
}

static sw_string* sw_run_impl(sw_string* cmd, sw_array* args, sw_string* input) {
    int out_pipe[2];
    int in_pipe[2] = {-1, -1};
    int64_t has_input = input != NULL && input->len > 0;
    if (pipe(out_pipe) != 0) {
        return sw_string_from_literal("", 0);
    }
    if (has_input && pipe(in_pipe) != 0) {
        close(out_pipe[0]);
        close(out_pipe[1]);
        return sw_string_from_literal("", 0);
    }
    char** argv = sw_build_argv(cmd, args);
    int pid = fork();
    if (pid < 0) {
        close(out_pipe[0]);
        close(out_pipe[1]);
        if (has_input) {
            close(in_pipe[0]);
            close(in_pipe[1]);
        }
        return sw_string_from_literal("", 0);
    }
    if (pid == 0) {
        dup2(out_pipe[1], 1);
        dup2(out_pipe[1], 2);
        if (has_input) {
            dup2(in_pipe[0], 0);
        }
        close(out_pipe[0]);
        close(out_pipe[1]);
        if (has_input) {
            close(in_pipe[0]);
            close(in_pipe[1]);
        }
        execvp(argv[0], argv);
        _exit(127);
    }
    close(out_pipe[1]);
    if (has_input) {
        close(in_pipe[0]);
        if (input->len > 0) {
            (void)write(in_pipe[1], input->data, (unsigned long)input->len);
        }
        close(in_pipe[1]);
    }
    char chunk[4096];
    char* buffer = (char*)malloc(4096);
    int64_t capacity = 4096;
    int64_t length = 0;
    while (1) {
        long got = read(out_pipe[0], chunk, sizeof(chunk));
        if (got <= 0) {
            break;
        }
        if (length + got > capacity) {
            capacity = (length + got) * 2;
            buffer = (char*)realloc(buffer, (sw_size)capacity);
        }
        memcpy(buffer + length, chunk, (uint64_t)got);
        length += got;
    }
    close(out_pipe[0]);
    int status = 0;
    waitpid(pid, &status, 0);
    sw_string* result = sw_string_from_literal(buffer, length);
    free(buffer);
    return result;
}

int64_t sw_run_status(sw_string* cmd, sw_array* args) {
    char** argv = sw_build_argv(cmd, args);
    int pid = fork();
    if (pid < 0) {
        return -1;
    }
    if (pid == 0) {
        execvp(argv[0], argv);
        _exit(127);
    }
    int status = 0;
    if (waitpid(pid, &status, 0) < 0) {
        return -1;
    }
    int code = status & 0x7f;
    if (code == 0) {
        return (status >> 8) & 0xff;
    }
    return 128 + code;
}

#endif

sw_string* sw_run(sw_string* cmd, sw_array* args) {
    return sw_run_impl(cmd, args, NULL);
}

sw_string* sw_run_with_input(sw_string* cmd, sw_array* args, sw_string* input) {
    return sw_run_impl(cmd, args, input);
}

sw_string* sw_platform(void) {
#if defined(_WIN32)
    return sw_string_from_literal("windows", 7);
#elif defined(__APPLE__)
    return sw_string_from_literal("macos", 5);
#else
    return sw_string_from_literal("linux", 5);
#endif
}

// ---------------------------------------------------------------------------
// 标准库扩充批次 A+B：编码 / 字符串补充 / 数学补充 / 时间补充 / io /
// os 系统信息 / fs 文件系统补充
// ---------------------------------------------------------------------------

// ---- 编码：base64 / hex / url ----

static const char sw_base64_chars[] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

sw_string* sw_base64_encode(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t out_len = ((text->len + 2) / 3) * 4;
    char* buffer = (char*)sw_gc_alloc((uint64_t)out_len + 1);
    int64_t out = 0;
    int64_t i = 0;
    while (i + 2 < text->len) {
        unsigned int value = ((unsigned char)text->data[i] << 16) |
                             ((unsigned char)text->data[i + 1] << 8) |
                             (unsigned char)text->data[i + 2];
        buffer[out++] = sw_base64_chars[(value >> 18) & 0x3F];
        buffer[out++] = sw_base64_chars[(value >> 12) & 0x3F];
        buffer[out++] = sw_base64_chars[(value >> 6) & 0x3F];
        buffer[out++] = sw_base64_chars[value & 0x3F];
        i += 3;
    }
    int64_t remaining = text->len - i;
    if (remaining == 1) {
        unsigned int value = (unsigned char)text->data[i] << 16;
        buffer[out++] = sw_base64_chars[(value >> 18) & 0x3F];
        buffer[out++] = sw_base64_chars[(value >> 12) & 0x3F];
        buffer[out++] = '=';
        buffer[out++] = '=';
    } else if (remaining == 2) {
        unsigned int value = ((unsigned char)text->data[i] << 16) |
                             ((unsigned char)text->data[i + 1] << 8);
        buffer[out++] = sw_base64_chars[(value >> 18) & 0x3F];
        buffer[out++] = sw_base64_chars[(value >> 12) & 0x3F];
        buffer[out++] = sw_base64_chars[(value >> 6) & 0x3F];
        buffer[out++] = '=';
    }
    buffer[out] = 0;
    return sw_string_from_literal(buffer, out);
}

static int sw_base64_value(char c) {
    if (c >= 'A' && c <= 'Z') {
        return c - 'A';
    }
    if (c >= 'a' && c <= 'z') {
        return c - 'a' + 26;
    }
    if (c >= '0' && c <= '9') {
        return c - '0' + 52;
    }
    if (c == '+') {
        return 62;
    }
    if (c == '/') {
        return 63;
    }
    return -1;
}

sw_string* sw_base64_decode(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len + 1);
    int64_t out = 0;
    int64_t i = 0;
    while (i < text->len) {
        int a = -1, b = -1, c = -1, d = -1;
        while (i < text->len && a < 0) {
            a = sw_base64_value(text->data[i++]);
        }
        while (i < text->len && b < 0) {
            b = sw_base64_value(text->data[i++]);
        }
        if (a < 0 || b < 0) {
            break;
        }
        while (i < text->len && c < 0) {
            if (text->data[i] == '=') {
                i++;
                break;
            }
            c = sw_base64_value(text->data[i++]);
        }
        while (i < text->len && d < 0) {
            if (text->data[i] == '=') {
                i++;
                break;
            }
            d = sw_base64_value(text->data[i++]);
        }
        buffer[out++] = (char)((a << 2) | (b >> 4));
        if (c >= 0) {
            buffer[out++] = (char)(((b & 0x0F) << 4) | (c >> 2));
        }
        if (d >= 0) {
            buffer[out++] = (char)(((c & 0x03) << 6) | d);
        }
        if (c < 0) {
            break;
        }
    }
    return sw_string_from_literal(buffer, out);
}

// ---- base32：RFC 4648 字母表 A-Z2-7，= 填充 ----

static const char sw_base32_chars[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

sw_string* sw_base32_encode(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t out_len = ((text->len + 4) / 5) * 8;
    char* buffer = (char*)sw_gc_alloc((uint64_t)out_len + 1);
    int64_t out = 0;
    int64_t i = 0;
    while (i + 4 < text->len) {
        uint64_t value = ((uint64_t)(unsigned char)text->data[i] << 32) |
                         ((uint64_t)(unsigned char)text->data[i + 1] << 24) |
                         ((uint64_t)(unsigned char)text->data[i + 2] << 16) |
                         ((uint64_t)(unsigned char)text->data[i + 3] << 8) |
                         (uint64_t)(unsigned char)text->data[i + 4];
        buffer[out++] = sw_base32_chars[(value >> 35) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 30) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 25) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 20) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 15) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 10) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 5) & 0x1F];
        buffer[out++] = sw_base32_chars[value & 0x1F];
        i += 5;
    }
    int64_t remaining = text->len - i;
    if (remaining == 1) {
        uint64_t value = (uint64_t)(unsigned char)text->data[i] << 32;
        buffer[out++] = sw_base32_chars[(value >> 35) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 30) & 0x1F];
        buffer[out++] = '=';
        buffer[out++] = '=';
        buffer[out++] = '=';
        buffer[out++] = '=';
        buffer[out++] = '=';
        buffer[out++] = '=';
    } else if (remaining == 2) {
        uint64_t value = ((uint64_t)(unsigned char)text->data[i] << 32) |
                         ((uint64_t)(unsigned char)text->data[i + 1] << 24);
        buffer[out++] = sw_base32_chars[(value >> 35) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 30) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 25) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 20) & 0x1F];
        buffer[out++] = '=';
        buffer[out++] = '=';
        buffer[out++] = '=';
        buffer[out++] = '=';
    } else if (remaining == 3) {
        uint64_t value = ((uint64_t)(unsigned char)text->data[i] << 32) |
                         ((uint64_t)(unsigned char)text->data[i + 1] << 24) |
                         ((uint64_t)(unsigned char)text->data[i + 2] << 16);
        buffer[out++] = sw_base32_chars[(value >> 35) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 30) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 25) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 20) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 15) & 0x1F];
        buffer[out++] = '=';
        buffer[out++] = '=';
        buffer[out++] = '=';
    } else if (remaining == 4) {
        uint64_t value = ((uint64_t)(unsigned char)text->data[i] << 32) |
                         ((uint64_t)(unsigned char)text->data[i + 1] << 24) |
                         ((uint64_t)(unsigned char)text->data[i + 2] << 16) |
                         ((uint64_t)(unsigned char)text->data[i + 3] << 8);
        buffer[out++] = sw_base32_chars[(value >> 35) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 30) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 25) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 20) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 15) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 10) & 0x1F];
        buffer[out++] = sw_base32_chars[(value >> 5) & 0x1F];
        buffer[out++] = '=';
    }
    buffer[out] = 0;
    return sw_string_from_literal(buffer, out);
}

static int sw_base32_value(char c) {
    if (c >= 'A' && c <= 'Z') {
        return c - 'A';
    }
    if (c >= '2' && c <= '7') {
        return c - '2' + 26;
    }
    return -1;
}

sw_string* sw_base32_decode(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len + 1);
    int64_t out = 0;
    int64_t i = 0;
    while (i < text->len) {
        int v[8];
        int count = 0;
        while (i < text->len && count < 8) {
            char c = text->data[i++];
            if (c == '=') {
                continue;
            }
            int val = sw_base32_value(c);
            if (val < 0) {
                count = -1;
                break;
            }
            v[count++] = val;
        }
        if (count < 0) {
            break;
        }
        if (count == 0) {
            continue;
        }
        // 每 8 个 5-bit 值还原 5 字节；不足时按位拼接可用部分。
        uint64_t value = 0;
        for (int k = 0; k < count; k++) {
            value = (value << 5) | (uint64_t)v[k];
        }
        // 对齐：凑满 40 位后输出 5 字节。
        if (count == 8) {
            buffer[out++] = (char)((value >> 32) & 0xFF);
            buffer[out++] = (char)((value >> 24) & 0xFF);
            buffer[out++] = (char)((value >> 16) & 0xFF);
            buffer[out++] = (char)((value >> 8) & 0xFF);
            buffer[out++] = (char)(value & 0xFF);
        } else {
            // 尾部：count 个 5-bit 值共 count*5 位，按字节数截取。
            int bits = count * 5;
            int bytes = bits / 8;
            value <<= (40 - bits);
            for (int k = 0; k < bytes; k++) {
                int shift = 32 - k * 8;
                buffer[out++] = (char)((value >> shift) & 0xFF);
            }
        }
    }
    return sw_string_from_literal(buffer, out);
}

static const char sw_hex_chars[] = "0123456789abcdef";

sw_string* sw_hex_encode(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len * 2 + 1);
    int64_t out = 0;
    for (int64_t i = 0; i < text->len; i++) {
        unsigned char byte = (unsigned char)text->data[i];
        buffer[out++] = sw_hex_chars[byte >> 4];
        buffer[out++] = sw_hex_chars[byte & 0x0F];
    }
    buffer[out] = 0;
    return sw_string_from_literal(buffer, out);
}

static int sw_hex_value(char c) {
    if (c >= '0' && c <= '9') {
        return c - '0';
    }
    if (c >= 'a' && c <= 'f') {
        return c - 'a' + 10;
    }
    if (c >= 'A' && c <= 'F') {
        return c - 'A' + 10;
    }
    return -1;
}

sw_string* sw_hex_decode(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len / 2 + 1);
    int64_t out = 0;
    int64_t i = 0;
    while (i + 1 < text->len) {
        int high = sw_hex_value(text->data[i]);
        int low = sw_hex_value(text->data[i + 1]);
        if (high < 0 || low < 0) {
            break;
        }
        buffer[out++] = (char)((high << 4) | low);
        i += 2;
    }
    return sw_string_from_literal(buffer, out);
}

static int sw_url_unreserved(unsigned char c) {
    return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') ||
           (c >= '0' && c <= '9') || c == '-' || c == '_' || c == '.' || c == '~';
}

sw_string* sw_url_encode(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len * 3 + 1);
    int64_t out = 0;
    for (int64_t i = 0; i < text->len; i++) {
        unsigned char c = (unsigned char)text->data[i];
        if (sw_url_unreserved(c)) {
            buffer[out++] = (char)c;
        } else {
            buffer[out++] = '%';
            buffer[out++] = sw_hex_chars[c >> 4];
            buffer[out++] = sw_hex_chars[c & 0x0F];
        }
    }
    buffer[out] = 0;
    return sw_string_from_literal(buffer, out);
}

sw_string* sw_url_decode(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len + 1);
    int64_t out = 0;
    int64_t i = 0;
    while (i < text->len) {
        if (text->data[i] == '%' && i + 2 < text->len) {
            int high = sw_hex_value(text->data[i + 1]);
            int low = sw_hex_value(text->data[i + 2]);
            if (high >= 0 && low >= 0) {
                buffer[out++] = (char)((high << 4) | low);
                i += 3;
                continue;
            }
        }
        buffer[out++] = text->data[i++];
    }
    return sw_string_from_literal(buffer, out);
}

// ---- 字符串补充 ----

int64_t sw_ends_with(sw_string* text, sw_string* suffix) {
    if (text == NULL || suffix == NULL) {
        return 0;
    }
    if (suffix->len > text->len) {
        return 0;
    }
    return memcmp(
        text->data + text->len - suffix->len,
        suffix->data,
        (uint64_t)suffix->len
    ) == 0
        ? 1
        : 0;
}

int64_t sw_is_ascii(sw_string* text) {
    if (text == NULL) {
        return 0;
    }
    for (int64_t i = 0; i < text->len; i++) {
        if ((unsigned char)text->data[i] >= 0x80) {
            return 0;
        }
    }
    return 1;
}

static int sw_is_space(char c) {
    return c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\v' || c == '\f';
}

sw_string* sw_trim_left(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t start = 0;
    while (start < text->len && sw_is_space(text->data[start])) {
        start++;
    }
    return sw_string_from_literal(text->data + start, text->len - start);
}

sw_string* sw_trim_right(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t end = text->len;
    while (end > 0 && sw_is_space(text->data[end - 1])) {
        end--;
    }
    return sw_string_from_literal(text->data, end);
}

sw_array* sw_lines(sw_string* text) {
    if (text == NULL || text->len == 0) {
        return sw_array_new(8, 0);
    }
    int64_t capacity = 1;
    for (int64_t i = 0; i < text->len; i++) {
        if (text->data[i] == '\n') {
            capacity++;
        }
    }
    sw_array* array = sw_array_new(8, capacity);
    int64_t slot = 0;
    int64_t start = 0;
    for (int64_t i = 0; i < text->len; i++) {
        if (text->data[i] == '\n') {
            int64_t len = i - start;
            if (len > 0 && text->data[start + len - 1] == '\r') {
                len--;
            }
            ((int64_t*)array->data)[slot++] =
                (int64_t)sw_string_from_literal(text->data + start, len);
            start = i + 1;
        }
    }
    // 末尾换行不产生空行；否则补最后一段。
    if (start < text->len) {
        int64_t len = text->len - start;
        if (len > 0 && text->data[start + len - 1] == '\r') {
            len--;
        }
        ((int64_t*)array->data)[slot++] =
            (int64_t)sw_string_from_literal(text->data + start, len);
    }
    array->len = slot;
    array->cap = slot;
    return array;
}

sw_array* sw_split_whitespace(sw_string* text) {
    if (text == NULL || text->len == 0) {
        return sw_array_new(8, 0);
    }
    int64_t capacity = text->len / 2 + 1;
    sw_array* array = sw_array_new(8, capacity);
    int64_t slot = 0;
    int64_t i = 0;
    while (i < text->len) {
        while (i < text->len && sw_is_space(text->data[i])) {
            i++;
        }
        int64_t start = i;
        while (i < text->len && !sw_is_space(text->data[i])) {
            i++;
        }
        if (i > start) {
            ((int64_t*)array->data)[slot++] =
                (int64_t)sw_string_from_literal(text->data + start, i - start);
        }
    }
    array->len = slot;
    array->cap = slot;
    return array;
}

int64_t sw_count(sw_string* text, sw_string* needle) {
    if (text == NULL || needle == NULL || needle->len == 0) {
        return 0;
    }
    int64_t count = 0;
    for (int64_t i = 0; i + needle->len <= text->len;) {
        int64_t ok = 1;
        for (int64_t j = 0; j < needle->len; j++) {
            if (text->data[i + j] != needle->data[j]) {
                ok = 0;
                break;
            }
        }
        if (ok) {
            count++;
            i += needle->len;
        } else {
            i++;
        }
    }
    return count;
}

int64_t sw_last_index_of(sw_string* text, sw_string* needle) {
    if (text == NULL || needle == NULL || needle->len == 0) {
        return -1;
    }
    int64_t found = -1;
    for (int64_t i = 0; i + needle->len <= text->len; i++) {
        int64_t ok = 1;
        for (int64_t j = 0; j < needle->len; j++) {
            if (text->data[i + j] != needle->data[j]) {
                ok = 0;
                break;
            }
        }
        if (ok) {
            found = i;
        }
    }
    return found;
}

sw_array* sw_chars(sw_string* text) {
    if (text == NULL) {
        return sw_array_new(8, 0);
    }
    int64_t count = utf8_len(text);
    sw_array* array = sw_array_new(8, count);
    int64_t slot = 0;
    int64_t offset = 0;
    while (offset < text->len) {
        int64_t char_len = sw_utf8_char_length(text->data, offset, text->len);
        ((int64_t*)array->data)[slot++] =
            (int64_t)sw_string_from_literal(text->data + offset, char_len);
        offset += char_len;
    }
    array->len = slot;
    array->cap = slot;
    return array;
}

sw_string* sw_from_utf8_bytes(sw_array* bytes) {
    if (bytes == NULL) {
        return sw_string_from_literal("", 0);
    }
    return sw_string_from_literal((const char*)bytes->data, bytes->len);
}

sw_array* sw_to_utf8_bytes(sw_string* text) {
    if (text == NULL) {
        return sw_array_new(1, 0);
    }
    sw_array* array = sw_array_new(1, text->len);
    if (text->len > 0) {
        memcpy(array->data, text->data, (uint64_t)text->len);
    }
    return array;
}

sw_string* sw_escape(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len * 4 + 1);
    int64_t out = 0;
    for (int64_t i = 0; i < text->len; i++) {
        unsigned char c = (unsigned char)text->data[i];
        switch (c) {
            case '"':
                buffer[out++] = '\\';
                buffer[out++] = '"';
                break;
            case '\\':
                buffer[out++] = '\\';
                buffer[out++] = '\\';
                break;
            case '\n':
                buffer[out++] = '\\';
                buffer[out++] = 'n';
                break;
            case '\r':
                buffer[out++] = '\\';
                buffer[out++] = 'r';
                break;
            case '\t':
                buffer[out++] = '\\';
                buffer[out++] = 't';
                break;
            default:
                if (c < 0x20 || c == 0x7F) {
                    buffer[out++] = '\\';
                    buffer[out++] = 'x';
                    buffer[out++] = sw_hex_chars[c >> 4];
                    buffer[out++] = sw_hex_chars[c & 0x0F];
                } else {
                    buffer[out++] = (char)c;
                }
        }
    }
    buffer[out] = 0;
    return sw_string_from_literal(buffer, out);
}

sw_string* sw_unescape(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len + 1);
    int64_t out = 0;
    int64_t i = 0;
    while (i < text->len) {
        if (text->data[i] == '\\' && i + 1 < text->len) {
            char next = text->data[i + 1];
            if (next == 'n') {
                buffer[out++] = '\n';
                i += 2;
                continue;
            }
            if (next == 'r') {
                buffer[out++] = '\r';
                i += 2;
                continue;
            }
            if (next == 't') {
                buffer[out++] = '\t';
                i += 2;
                continue;
            }
            if (next == '"') {
                buffer[out++] = '"';
                i += 2;
                continue;
            }
            if (next == '\\') {
                buffer[out++] = '\\';
                i += 2;
                continue;
            }
            if (next == 'x' && i + 3 < text->len) {
                int high = sw_hex_value(text->data[i + 2]);
                int low = sw_hex_value(text->data[i + 3]);
                if (high >= 0 && low >= 0) {
                    buffer[out++] = (char)((high << 4) | low);
                    i += 4;
                    continue;
                }
            }
        }
        buffer[out++] = text->data[i++];
    }
    return sw_string_from_literal(buffer, out);
}

// ---- 数学补充（sign / 随机浮点 / 常量）----

double sw_sign(double value) {
    return value < 0 ? -1.0 : (value > 0 ? 1.0 : 0.0);
}

double sw_rand_float(void) {
    extern int rand(void);
    return (double)rand() / 2147483647.0;
}

double sw_rand_range(double min, double max) {
    if (min >= max) {
        return min;
    }
    return min + sw_rand_float() * (max - min);
}

double sw_pi(void) {
    return 3.14159265358979323846;
}

double sw_e(void) {
    return 2.71828182845904523536;
}

// ---- 时间补充：time_format / time_from_parts / timezone_offset_sec ----

static const char* sw_month_short[12] = {
    "Jan", "Feb", "Mar", "Apr", "May", "Jun",
    "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
};
static const char* sw_month_long[12] = {
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
};
static const char* sw_wday_short[7] = {
    "Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat",
};
static const char* sw_wday_long[7] = {
    "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday",
};

static void sw_append_str(char* buffer, int64_t* used, const char* text, int64_t len) {
    if (*used + len < 256) {
        memcpy(buffer + *used, text, (uint64_t)len);
        *used += len;
    }
}

sw_string* sw_time_format(int64_t seconds, sw_string* fmt) {
    if (fmt == NULL) {
        return sw_string_from_literal("", 0);
    }
    int year = 1970, month = 1, day = 1, hour = 0, minute = 0, second = 0, wday = 0;
#if defined(_WIN32)
    unsigned char st[16];
    sw_unix_to_local_systemtime(st, seconds);
    year = *(unsigned short*)st;
    month = *(unsigned short*)(st + 2);
    wday = *(unsigned short*)(st + 4);
    day = *(unsigned short*)(st + 6);
    hour = *(unsigned short*)(st + 8);
    minute = *(unsigned short*)(st + 10);
    second = *(unsigned short*)(st + 12);
#else
    extern void* localtime_r(const void* time, void* tm);
    unsigned char tm[64];
    unsigned char t[8];
    *(int64_t*)t = seconds;
    if (localtime_r(t, tm) == NULL) {
        return sw_string_from_literal("", 0);
    }
    second = *(int*)(tm + 0);
    minute = *(int*)(tm + 4);
    hour = *(int*)(tm + 8);
    day = *(int*)(tm + 12);
    month = *(int*)(tm + 16) + 1;
    year = *(int*)(tm + 20) + 1900;
    wday = *(int*)(tm + 24);
#endif
    char buffer[256];
    int64_t used = 0;
    char part[64];
    int64_t i = 0;
    while (i < fmt->len) {
        if (fmt->data[i] != '%' || i + 1 >= fmt->len) {
            if (used < 255) {
                buffer[used++] = fmt->data[i++];
            } else {
                i++;
            }
            continue;
        }
        char conv = fmt->data[i + 1];
        int len = 0;
        switch (conv) {
            case 'Y':
                len = snprintf(part, sizeof(part), "%04d", year);
                break;
            case 'y':
                len = snprintf(part, sizeof(part), "%02d", year % 100);
                break;
            case 'm':
                len = snprintf(part, sizeof(part), "%02d", month);
                break;
            case 'd':
                len = snprintf(part, sizeof(part), "%02d", day);
                break;
            case 'H':
                len = snprintf(part, sizeof(part), "%02d", hour);
                break;
            case 'M':
                len = snprintf(part, sizeof(part), "%02d", minute);
                break;
            case 'S':
                len = snprintf(part, sizeof(part), "%02d", second);
                break;
            case 'e':
                len = snprintf(part, sizeof(part), "%2d", day);
                break;
            case 'p':
                len = snprintf(part, sizeof(part), "%s", hour < 12 ? "AM" : "PM");
                break;
            case 'a':
                len = snprintf(part, sizeof(part), "%s", sw_wday_short[wday]);
                break;
            case 'A':
                len = snprintf(part, sizeof(part), "%s", sw_wday_long[wday]);
                break;
            case 'b':
                len = snprintf(part, sizeof(part), "%s", sw_month_short[month - 1]);
                break;
            case 'B':
                len = snprintf(part, sizeof(part), "%s", sw_month_long[month - 1]);
                break;
            case '%':
                part[0] = '%';
                len = 1;
                break;
            default:
                part[0] = '%';
                part[1] = conv;
                len = 2;
        }
        sw_append_str(buffer, &used, part, len);
        i += 2;
    }
    return sw_string_from_literal(buffer, used);
}

int64_t sw_time_from_parts(
    int64_t year,
    int64_t month,
    int64_t day,
    int64_t hour,
    int64_t minute,
    int64_t second
) {
#if defined(_WIN32)
    extern int SystemTimeToFileTime(const void* system_time, void* file_time);
    extern int LocalFileTimeToFileTime(const void* local_file_time, void* file_time);
    unsigned char st[16];
    memset(st, 0, sizeof(st));
    *(unsigned short*)(st + 0) = (unsigned short)year;
    *(unsigned short*)(st + 2) = (unsigned short)month;
    *(unsigned short*)(st + 6) = (unsigned short)day;
    *(unsigned short*)(st + 8) = (unsigned short)hour;
    *(unsigned short*)(st + 10) = (unsigned short)minute;
    *(unsigned short*)(st + 12) = (unsigned short)second;
    unsigned char local_ft[8];
    unsigned char ft[8];
    if (!SystemTimeToFileTime(st, local_ft) || !LocalFileTimeToFileTime(local_ft, ft)) {
        return -1;
    }
    uint64_t since_1601 =
        ((uint64_t)(*(unsigned int*)(ft + 4)) << 32) | (*(unsigned int*)ft);
    return (int64_t)((since_1601 - 116444736000000000ULL) / 10000000ULL);
#else
    extern long mktime(void* tm);
    unsigned char tm[64];
    memset(tm, 0, sizeof(tm));
    *(int*)(tm + 0) = (int)second;
    *(int*)(tm + 4) = (int)minute;
    *(int*)(tm + 8) = (int)hour;
    *(int*)(tm + 12) = (int)day;
    *(int*)(tm + 16) = (int)month - 1;
    *(int*)(tm + 20) = (int)year - 1900;
    *(int*)(tm + 32) = -1;  // 自动判断夏令时
    return (int64_t)mktime(tm);
#endif
}

int64_t sw_timezone_offset_sec(void) {
#if defined(_WIN32)
    // TIME_ZONE_INFORMATION：bias(4) StandardName(64) StandardDate(16)
    // StandardBias(4) DaylightName(64) DaylightDate(16) DaylightBias(4)
    extern int GetTimeZoneInformation(void* tzi);
    unsigned char tzi[172];
    memset(tzi, 0, sizeof(tzi));
    int result = GetTimeZoneInformation(tzi);
    long bias = *(long*)(tzi + 0);
    long daylight_bias = *(long*)(tzi + 168);
    // 返回值 2 = 夏令时生效中：偏移 = -(bias + daylight_bias) 分钟。
    long total_minutes = result == 2 ? bias + daylight_bias : bias;
    return -(int64_t)total_minutes * 60;
#else
    extern void* localtime_r(const void* time, void* tm);
    extern void* gmtime_r(const void* time, void* tm);
    extern long mktime(void* tm);
    int64_t now = now_sec();
    unsigned char t[8];
    *(int64_t*)t = now;
    unsigned char local_tm[64];
    unsigned char utc_tm[64];
    if (!localtime_r(t, local_tm) || !gmtime_r(t, utc_tm)) {
        return 0;
    }
    *(int*)(local_tm + 32) = -1;
    *(int*)(utc_tm + 32) = -1;
    long local_epoch = mktime(local_tm);
    long utc_epoch = mktime(utc_tm);
    return (int64_t)(local_epoch - utc_epoch);
#endif
}

// ---- io 补充：stderr 输出 / 读全部 stdin ----

void sw_eprintln(sw_string* text) {
    if (text != NULL && text->len > 0) {
        fwrite(text->data, 1, (uint64_t)text->len, stderr);
    }
    fputc('\n', stderr);
}

void sw_eprint(sw_string* text) {
    if (text != NULL && text->len > 0) {
        fwrite(text->data, 1, (uint64_t)text->len, stderr);
    }
}

sw_string* sw_read_all_stdin(void) {
    char chunk[4096];
    char* buffer = (char*)malloc(4096);
    int64_t capacity = 4096;
    int64_t length = 0;
    while (1) {
        uint64_t got = fread(chunk, 1, sizeof(chunk), stdin);
        if (got == 0) {
            break;
        }
        if (length + (int64_t)got > capacity) {
            capacity = (length + (int64_t)got) * 2;
            buffer = (char*)realloc(buffer, (sw_size)capacity);
        }
        memcpy(buffer + length, chunk, got);
        length += (int64_t)got;
    }
    sw_string* result = sw_string_from_literal(buffer, length);
    free(buffer);
    return result;
}

// ---- os 补充：系统信息与目录 ----

sw_string* sw_cwd(void) {
    char buffer[4096];
#if defined(_WIN32)
    extern unsigned int GetCurrentDirectoryA(unsigned int size, char* buffer);
    unsigned int len = GetCurrentDirectoryA(sizeof(buffer), buffer);
    return sw_string_from_literal(buffer, (int64_t)len);
#else
    extern char* getcwd(char* buffer, unsigned long size);
    if (getcwd(buffer, sizeof(buffer)) == NULL) {
        return sw_string_from_literal("", 0);
    }
    return sw_string_from_literal(buffer, (int64_t)strlen(buffer));
#endif
}

int64_t sw_chdir(sw_string* path) {
#if defined(_WIN32)
    extern int SetCurrentDirectoryA(const char* path);
    return SetCurrentDirectoryA(path->data) ? 0 : -1;
#else
    extern int chdir(const char* path);
    return chdir(path->data) == 0 ? 0 : -1;
#endif
}

sw_string* sw_temp_dir(void) {
#if defined(_WIN32)
    extern unsigned int GetTempPathA(unsigned int size, char* buffer);
    char buffer[4096];
    unsigned int len = GetTempPathA(sizeof(buffer), buffer);
    return sw_string_from_literal(buffer, (int64_t)len);
#else
    extern char* getenv(const char* name);
    const char* dir = getenv("TMPDIR");
    if (dir == NULL || dir[0] == 0) {
        dir = "/tmp";
    }
    return sw_string_from_literal(dir, (int64_t)strlen(dir));
#endif
}

sw_string* sw_home_dir(void) {
    extern char* getenv(const char* name);
#if defined(_WIN32)
    const char* home = getenv("USERPROFILE");
#else
    const char* home = getenv("HOME");
#endif
    if (home == NULL) {
        return sw_string_from_literal("", 0);
    }
    return sw_string_from_literal(home, (int64_t)strlen(home));
}

sw_string* sw_hostname(void) {
    char buffer[4096];
#if defined(_WIN32)
    extern int GetComputerNameA(char* buffer, unsigned int* size);
    unsigned int size = sizeof(buffer);
    if (!GetComputerNameA(buffer, &size)) {
        return sw_string_from_literal("", 0);
    }
    return sw_string_from_literal(buffer, (int64_t)strlen(buffer));
#else
    extern int gethostname(char* name, unsigned long size);
    if (gethostname(buffer, sizeof(buffer)) != 0) {
        return sw_string_from_literal("", 0);
    }
    return sw_string_from_literal(buffer, (int64_t)strlen(buffer));
#endif
}

int64_t sw_cpu_count(void) {
#if defined(_WIN32)
    extern void GetSystemInfo(void* info);
    unsigned char info[56];
    memset(info, 0, sizeof(info));
    GetSystemInfo(info);
    return *(unsigned int*)(info + 40);
#else
    extern long sysconf(int name);
#if defined(__APPLE__)
    long count = sysconf(58);  // _SC_NPROCESSORS_ONLN（macOS）
#else
    long count = sysconf(84);  // _SC_NPROCESSORS_ONLN（Linux/musl）
#endif
    return count > 0 ? count : 1;
#endif
}

sw_array* sw_env_keys(void) {
    sw_array* array = sw_array_new(8, 16);
    int64_t slot = 0;
#if defined(_WIN32)
    extern char* GetEnvironmentStringsA(void);
    extern int FreeEnvironmentStringsA(char* block);
    char* block = GetEnvironmentStringsA();
    if (block != NULL) {
        char* cursor = block;
        while (cursor[0] != 0) {
            int64_t len = 0;
            while (cursor[len] != 0 && cursor[len] != '=') {
                len++;
            }
            if (len > 0) {
                if (slot >= array->len) {
                    sw_array* bigger = sw_array_new(8, array->len * 2 + 1);
                    for (int64_t i = 0; i < slot; i++) {
                        ((int64_t*)bigger->data)[i] = ((int64_t*)array->data)[i];
                    }
                    array = bigger;
                }
                ((int64_t*)array->data)[slot++] =
                    (int64_t)sw_string_from_literal(cursor, len);
            }
            while (cursor[len] != 0) {
                len++;
            }
            cursor += len + 1;
        }
        FreeEnvironmentStringsA(block);
    }
#else
    for (int64_t index = 0; environ[index] != NULL; index++) {
        const char* entry = environ[index];
        int64_t len = 0;
        while (entry[len] != 0 && entry[len] != '=') {
            len++;
        }
        if (len > 0) {
            if (slot >= array->len) {
                sw_array* bigger = sw_array_new(8, array->len * 2 + 1);
                for (int64_t i = 0; i < slot; i++) {
                    ((int64_t*)bigger->data)[i] = ((int64_t*)array->data)[i];
                }
                array = bigger;
            }
            ((int64_t*)array->data)[slot++] =
                (int64_t)sw_string_from_literal(entry, len);
        }
    }
#endif
    array->len = slot;
    array->cap = slot;
    return array;
}

int64_t sw_setenv(sw_string* name, sw_string* value) {
#if defined(_WIN32)
    extern int SetEnvironmentVariableA(const char* name, const char* value);
    return SetEnvironmentVariableA(name->data, value->data) ? 0 : -1;
#else
    extern int setenv(const char* name, const char* value, int overwrite);
    return setenv(name->data, value->data, 1) == 0 ? 0 : -1;
#endif
}

// ---- fs 补充：文件信息 / 权限 / 递归操作 / glob ----

int64_t sw_file_size_path(sw_string* path) {
    sw_file_handle* file = fopen(path->data, "rb");
    if (file == NULL) {
        return -1;
    }
    fseek(file, 0, 2);
    long size = ftell(file);
    fclose(file);
    return size < 0 ? -1 : (int64_t)size;
}

// stat 结构体偏移按平台区分：Apple（macOS）的 mode 在 4、mtime 在 48；
// Linux/musl（x86_64/aarch64 同布局）mode 在 24、mtime 在 88。
// Windows 不使用 stat（走 GetFileAttributesExA），不受此影响。
#if defined(__APPLE__)
#define SW_STAT_MODE_OFFSET 4
#define SW_STAT_MTIME_OFFSET 48
#else
#define SW_STAT_MODE_OFFSET 24
#define SW_STAT_MTIME_OFFSET 88
#endif

int64_t sw_file_mtime(sw_string* path) {
#if defined(_WIN32)
    extern int GetFileAttributesExA(const char* path, int level, void* data);
    unsigned char data[36];
    memset(data, 0, sizeof(data));
    if (!GetFileAttributesExA(path->data, 0, data)) {
        return -1;
    }
    uint64_t since_1601 =
        ((uint64_t)(*(unsigned int*)(data + 24)) << 32) | (*(unsigned int*)(data + 20));
    return (int64_t)((since_1601 - 116444736000000000ULL) / 10000000ULL);
#else
    extern int stat(const char* path, void* buf);
    unsigned char buf[160];
    memset(buf, 0, sizeof(buf));
    if (stat(path->data, buf) != 0) {
        return -1;
    }
    return *(int64_t*)(buf + SW_STAT_MTIME_OFFSET);
#endif
}

int64_t sw_is_file(sw_string* path) {
#if defined(_WIN32)
    extern unsigned int GetFileAttributesA(const char* path);
    unsigned int attrs = GetFileAttributesA(path->data);
    if (attrs == 0xFFFFFFFFu) {
        return 0;
    }
    return (attrs & 0x10) ? 0 : 1;
#else
    extern int stat(const char* path, void* buf);
    unsigned char buf[160];
    memset(buf, 0, sizeof(buf));
    if (stat(path->data, buf) != 0) {
        return 0;
    }
    unsigned int mode = *(unsigned int*)(buf + SW_STAT_MODE_OFFSET);
    return (mode & 0xF000) == 0x8000 ? 1 : 0;
#endif
}

int64_t sw_chmod(sw_string* path, int64_t mode) {
#if defined(_WIN32)
    extern int _chmod(const char* path, int mode);
    // POSIX 用户写位是 0o200（十进制 128）；MSVC 的 _S_IWRITE 是 0x80，_S_IREAD 是 0x100。
    int windows_mode = (mode & 128) ? 0x80 : 0x100;
    return _chmod(path->data, windows_mode) == 0 ? 0 : -1;
#else
    extern int chmod(const char* path, unsigned int mode);
    return chmod(path->data, (unsigned int)mode) == 0 ? 0 : -1;
#endif
}

int64_t sw_touch(sw_string* path) {
    sw_file_handle* file = fopen(path->data, "ab");
    if (file == NULL) {
        return -1;
    }
    fclose(file);
    return 0;
}

// 是否可写：Windows 看只读属性，POSIX 用 access(W_OK)。
int64_t sw_is_writable(sw_string* path) {
#if defined(_WIN32)
    extern unsigned int GetFileAttributesA(const char* path);
    unsigned int attrs = GetFileAttributesA(path->data);
    if (attrs == 0xFFFFFFFFu) {  // INVALID_FILE_ATTRIBUTES
        return 0;
    }
    return (attrs & 0x1u) ? 0 : 1;  // FILE_ATTRIBUTE_READONLY
#else
    extern int access(const char* path, int mode);
    return access(path->data, 2 /*W_OK*/) == 0 ? 1 : 0;
#endif
}

int64_t sw_copy_dir(sw_string* src, sw_string* dst) {
    if (!is_dir(src)) {
        return -1;
    }
    if (sw_mkdir(dst) != 0 && !is_dir(dst)) {
        return -1;
    }
    sw_array* entries = list_dir(src);
    for (int64_t i = 0; i < entries->len; i++) {
        sw_string* name = (sw_string*)((int64_t*)entries->data)[i];
        sw_string* full_src = path_join(src, name);
        sw_string* full_dst = path_join(dst, name);
        if (is_dir(full_src)) {
            if (sw_copy_dir(full_src, full_dst) != 0) {
                return -1;
            }
        } else {
            if (copy_file(full_src, full_dst) < 0) {
                return -1;
            }
        }
    }
    return 0;
}

int64_t sw_remove_all(sw_string* path) {
    if (is_dir(path)) {
        sw_array* entries = list_dir(path);
        for (int64_t i = 0; i < entries->len; i++) {
            sw_string* name = (sw_string*)((int64_t*)entries->data)[i];
            sw_string* full = path_join(path, name);
            if (sw_remove_all(full) != 0) {
                return -1;
            }
        }
    }
    return sw_remove(path);
}

static int sw_glob_match(const char* pattern, int64_t plen, const char* text, int64_t tlen) {
    int64_t p = 0;
    int64_t t = 0;
    int64_t star_p = -1;
    int64_t star_t = 0;
    while (t < tlen) {
        if (p < plen && (pattern[p] == '?' || pattern[p] == text[t])) {
            p++;
            t++;
        } else if (p < plen && pattern[p] == '*') {
            star_p = p++;
            star_t = t;
        } else if (star_p >= 0) {
            p = star_p + 1;
            t = ++star_t;
        } else {
            return 0;
        }
    }
    while (p < plen && pattern[p] == '*') {
        p++;
    }
    return p == plen;
}

sw_array* sw_glob(sw_string* pattern) {
    sw_string* dir = path_dirname(pattern);
    sw_string* base = path_basename(pattern);
    sw_array* result = sw_array_new(8, 0);
    int64_t slot = 0;
    sw_array* entries = list_dir(dir);
    for (int64_t i = 0; i < entries->len; i++) {
        sw_string* name = (sw_string*)((int64_t*)entries->data)[i];
        if (sw_glob_match(base->data, base->len, name->data, name->len)) {
            if (slot >= result->len) {
                sw_array* bigger = sw_array_new(8, result->len * 2 + 1);
                for (int64_t j = 0; j < slot; j++) {
                    ((int64_t*)bigger->data)[j] = ((int64_t*)result->data)[j];
                }
                result = bigger;
            }
            ((int64_t*)result->data)[slot++] = (int64_t)path_join(dir, name);
        }
    }
    result->len = slot;
    result->cap = slot;
    return result;
}

// ---------------------------------------------------------------------------
// 标准库扩充批次 A+B：目录定位 / 路径工具 / 磁盘与链接 / 字符串截断 /
// 数学角度与判定 / parse_datetime / base64url / html_escape / JSON 序列化
// ---------------------------------------------------------------------------

// ---- os：用户目录定位 / 用户名 / pid / 架构 / unsetenv ----

#if defined(_WIN32)
static sw_string* sw_wide_to_utf8(const void* wide) {
    extern int WideCharToMultiByte(
        unsigned int cp,
        unsigned long flags,
        const void* wstr,
        int wlen,
        char* out,
        int outlen,
        const void* def,
        const void* used
    );
    int len = WideCharToMultiByte(65001, 0, wide, -1, NULL, 0, NULL, NULL);
    if (len <= 0) {
        len = 1;
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)len);
    WideCharToMultiByte(65001, 0, wide, -1, buffer, len, NULL, NULL);
    return sw_string_from_literal(buffer, (int64_t)(len - 1));
}

static sw_string* sw_known_folder(const unsigned char rfid[16], const char* fallback) {
    extern int SHGetKnownFolderPath(const void* rfid, unsigned int flags, void* token, void** path);
    extern void CoTaskMemFree(void* ptr);
    void* wide = NULL;
    if (SHGetKnownFolderPath(rfid, 0, NULL, &wide) != 0 || wide == NULL) {
        // 已知文件夹不存在（如本机无 Videos）时回退到主目录下同名目录。
        sw_string* home = sw_home_dir();
        if (home->len == 0) {
            return sw_string_from_literal("", 0);
        }
        return path_join(home, sw_string_from_literal(fallback, (int64_t)strlen(fallback)));
    }
    sw_string* result = sw_wide_to_utf8(wide);
    CoTaskMemFree(wide);
    return result;
}
#endif

#if !defined(_WIN32)
static sw_string* sw_xdg_or_home(const char* xdg_name, const char* fallback) {
    extern char* getenv(const char* name);
    const char* xdg = getenv(xdg_name);
    if (xdg != NULL && xdg[0] != 0) {
        return sw_string_from_literal(xdg, (int64_t)strlen(xdg));
    }
    sw_string* home = sw_home_dir();
    if (home->len == 0) {
        return sw_string_from_literal("", 0);
    }
    return path_join(home, sw_string_from_literal(fallback, (int64_t)strlen(fallback)));
}
#endif

sw_string* sw_desktop_dir(void) {
#if defined(_WIN32)
    static const unsigned char rfid[16] = {
        0x3A, 0xCC, 0xBF, 0xB4, 0x2C, 0xDB, 0x4C, 0x42,
        0xB0, 0x29, 0x7F, 0xE9, 0x9A, 0x87, 0xC6, 0x41,
    };
    return sw_known_folder(rfid, "Desktop");
#else
    return sw_xdg_or_home("XDG_DESKTOP_DIR", "Desktop");
#endif
}

sw_string* sw_documents_dir(void) {
#if defined(_WIN32)
    static const unsigned char rfid[16] = {
        0xD0, 0x9A, 0xD3, 0xFD, 0x8F, 0x23, 0xAF, 0x46,
        0xAD, 0xB4, 0x6C, 0x85, 0x48, 0x03, 0x69, 0xC7,
    };
    return sw_known_folder(rfid, "Documents");
#else
    return sw_xdg_or_home("XDG_DOCUMENTS_DIR", "Documents");
#endif
}

sw_string* sw_downloads_dir(void) {
#if defined(_WIN32)
    static const unsigned char rfid[16] = {
        0x90, 0xE2, 0x4D, 0x37, 0x3F, 0x12, 0x65, 0x45,
        0x91, 0x64, 0x39, 0xC4, 0x92, 0x5E, 0x46, 0x7B,
    };
    return sw_known_folder(rfid, "Downloads");
#else
    return sw_xdg_or_home("XDG_DOWNLOAD_DIR", "Downloads");
#endif
}

sw_string* sw_pictures_dir(void) {
#if defined(_WIN32)
    static const unsigned char rfid[16] = {
        0x30, 0x81, 0xE2, 0x33, 0x1E, 0x4E, 0x76, 0x46,
        0x83, 0x5A, 0x98, 0x39, 0x5C, 0x3B, 0xC3, 0xBB,
    };
    return sw_known_folder(rfid, "Pictures");
#else
    return sw_xdg_or_home("XDG_PICTURES_DIR", "Pictures");
#endif
}

sw_string* sw_music_dir(void) {
#if defined(_WIN32)
    static const unsigned char rfid[16] = {
        0x71, 0xD5, 0xD8, 0x4B, 0x19, 0x6D, 0xD3, 0x48,
        0xBE, 0x97, 0x42, 0x22, 0x20, 0x08, 0x0E, 0x43,
    };
    return sw_known_folder(rfid, "Music");
#else
    return sw_xdg_or_home("XDG_MUSIC_DIR", "Music");
#endif
}

sw_string* sw_videos_dir(void) {
#if defined(_WIN32)
    static const unsigned char rfid[16] = {
        0x1D, 0x9B, 0x98, 0x18, 0xB5, 0x99, 0x5B, 0x45,
        0x84, 0x1C, 0xAB, 0x7C, 0x74, 0xE4, 0xDD, 0xF3,
    };
    return sw_known_folder(rfid, "Videos");
#else
    return sw_xdg_or_home("XDG_VIDEOS_DIR", "Videos");
#endif
}

sw_string* sw_config_dir(void) {
    extern char* getenv(const char* name);
#if defined(_WIN32)
    const char* appdata = getenv("APPDATA");
    if (appdata == NULL || appdata[0] == 0) {
        return sw_string_from_literal("", 0);
    }
    return sw_string_from_literal(appdata, (int64_t)strlen(appdata));
#else
    return sw_xdg_or_home("XDG_CONFIG_HOME", ".config");
#endif
}

sw_string* sw_system_dir(void) {
#if defined(_WIN32)
    extern unsigned int GetSystemDirectoryA(char* buffer, unsigned int size);
    char buffer[4096];
    unsigned int len = GetSystemDirectoryA(buffer, sizeof(buffer));
    return sw_string_from_literal(buffer, (int64_t)len);
#elif defined(__APPLE__)
    return sw_string_from_literal("/System", 7);
#else
    return sw_string_from_literal("/usr", 4);
#endif
}

sw_string* sw_username(void) {
    extern char* getenv(const char* name);
#if defined(_WIN32)
    const char* user = getenv("USERNAME");
#else
    const char* user = getenv("USER");
    if (user == NULL || user[0] == 0) {
        user = getenv("LOGNAME");
    }
#endif
    if (user == NULL) {
        return sw_string_from_literal("", 0);
    }
    return sw_string_from_literal(user, (int64_t)strlen(user));
}

int64_t sw_pid(void) {
#if defined(_WIN32)
    extern unsigned int GetCurrentProcessId(void);
    return (int64_t)GetCurrentProcessId();
#else
    extern int getpid(void);
    return (int64_t)getpid();
#endif
}

// 当前进程内存占用（KB）。
// Windows：K32GetProcessMemoryInfo（kernel32，Win7+）读 WorkingSetSize。
// POSIX：getrusage(RUSAGE_SELF) 的 ru_maxrss（Linux 单位 KB；macOS 单位字节）。
int64_t sw_memory_usage_kb(void) {
#if defined(_WIN32)
    extern void* GetCurrentProcess(void);
    extern int K32GetProcessMemoryInfo(void* process, void* counters, unsigned int cb);
    unsigned char counters[72];
    memset(counters, 0, sizeof(counters));
    if (K32GetProcessMemoryInfo(GetCurrentProcess(), counters, sizeof(counters)) != 0) {
        uint64_t working_set = *(uint64_t*)(counters + 16);  // WorkingSetSize
        return (int64_t)(working_set / 1024);
    }
    return -1;
#else
    extern int getrusage(int who, void* usage);
    unsigned char usage[256];
    memset(usage, 0, sizeof(usage));
    if (getrusage(0 /*RUSAGE_SELF*/, usage) == 0) {
        long maxrss = *(long*)(usage + 32);  // ru_maxrss
#if defined(__APPLE__)
        return (int64_t)(maxrss / 1024);     // macOS 单位字节
#else
        return (int64_t)maxrss;              // Linux 单位 KB
#endif
    }
    return -1;
#endif
}

sw_string* sw_arch(void) {
#if defined(__aarch64__)
    return sw_string_from_literal("aarch64", 7);
#else
    return sw_string_from_literal("x86_64", 6);
#endif
}

int64_t sw_unsetenv(sw_string* name) {
#if defined(_WIN32)
    extern int SetEnvironmentVariableA(const char* name, const char* value);
    return SetEnvironmentVariableA(name->data, NULL) ? 0 : -1;
#else
    extern int unsetenv(const char* name);
    return unsetenv(name->data) == 0 ? 0 : -1;
#endif
}

// ---- fs：路径工具 / mkdir_p / 磁盘 / 符号链接 / 权限 ----

static int sw_is_sep(char c) {
    return c == '/' || c == '\\';
}

int64_t sw_is_absolute(sw_string* path) {
#if defined(_WIN32)
    if (path->len >= 3 && path->data[1] == ':' && sw_is_sep(path->data[2])) {
        return 1;
    }
    if (path->len >= 2 && sw_is_sep(path->data[0]) && sw_is_sep(path->data[1])) {
        return 1;
    }
    return 0;
#else
    return path->len > 0 && path->data[0] == '/' ? 1 : 0;
#endif
}

sw_string* sw_path_normalize(sw_string* path) {
    if (path == NULL || path->len == 0) {
        return sw_string_from_literal("", 0);
    }
    char* input = (char*)sw_gc_alloc((uint64_t)path->len + 1);
    memcpy(input, path->data, (uint64_t)path->len);
    input[path->len] = 0;
#if defined(_WIN32)
    for (int64_t i = 0; i < path->len; i++) {
        if (input[i] == '\\') {
            input[i] = '/';
        }
    }
#endif
    int64_t drive = 0;
#if defined(_WIN32)
    if (path->len >= 2 && input[1] == ':') {
        drive = 2;
        if (path->len >= 3 && input[2] == '/') {
            drive = 3;
        }
    }
#endif
    int64_t absolute = 0;
    if (drive == 0 && path->len > 0 && input[0] == '/') {
        absolute = 1;
    }
    int64_t segments[256];
    int64_t count = 0;
    int64_t i = drive;
    while (i < path->len) {
        while (i < path->len && input[i] == '/') {
            i++;
        }
        int64_t start = i;
        while (i < path->len && input[i] != '/') {
            i++;
        }
        if (i > start) {
            int64_t seg_len = i - start;
            if (seg_len == 1 && input[start] == '.') {
                continue;
            }
            if (seg_len == 2 && input[start] == '.' && input[start + 1] == '.') {
                if (count > 0) {
                    count--;
                } else if (!absolute && count < 256) {
                    segments[count++] = start;
                }
                continue;
            }
            if (count < 256) {
                segments[count++] = start;
            }
        }
    }
    if (count == 0 && !absolute && drive == 0) {
        return sw_string_from_literal(".", 1);
    }
    int64_t total = drive + (absolute ? 1 : 0);
    for (int64_t k = 0; k < count; k++) {
        int64_t seg_len = 0;
        while (segments[k] + seg_len < path->len && input[segments[k] + seg_len] != '/') {
            seg_len++;
        }
        total += seg_len + 1;
    }
    char* out = (char*)sw_gc_alloc((uint64_t)total + 1);
    int64_t o = 0;
    char sep = '/';
#if defined(_WIN32)
    sep = '\\';
    for (int64_t k = 0; k < drive && k < path->len; k++) {
        out[o++] = input[k] == '/' ? sep : input[k];
    }
#else
    (void)drive;
#endif
    if (absolute && o == 0) {
        out[o++] = sep;
    }
    for (int64_t k = 0; k < count; k++) {
        if (o > 0 && out[o - 1] != sep) {
            out[o++] = sep;
        }
        int64_t seg_len = 0;
        while (segments[k] + seg_len < path->len && input[segments[k] + seg_len] != '/') {
            seg_len++;
        }
        memcpy(out + o, input + segments[k], (uint64_t)seg_len);
        o += seg_len;
    }
    out[o] = 0;
    return sw_string_from_literal(out, o);
}

sw_string* sw_path_absolute(sw_string* path) {
    if (sw_is_absolute(path)) {
        return sw_path_normalize(path);
    }
    sw_string* cwd = sw_cwd();
    if (cwd->len == 0) {
        return sw_string_from_literal(path->data, path->len);
    }
    return sw_path_normalize(path_join(cwd, path));
}

sw_array* sw_path_parts(sw_string* path) {
    sw_array* array = sw_array_new(8, 8);
    int64_t slot = 0;
    int64_t i = 0;
    while (i < path->len) {
        while (i < path->len && sw_is_sep(path->data[i])) {
            i++;
        }
        int64_t start = i;
        while (i < path->len && !sw_is_sep(path->data[i])) {
            i++;
        }
        if (i > start) {
            if (slot >= array->len) {
                sw_array* bigger = sw_array_new(8, array->len * 2 + 1);
                for (int64_t k = 0; k < slot; k++) {
                    ((int64_t*)bigger->data)[k] = ((int64_t*)array->data)[k];
                }
                array = bigger;
            }
            ((int64_t*)array->data)[slot++] =
                (int64_t)sw_string_from_literal(path->data + start, i - start);
        }
    }
    array->len = slot;
    array->cap = slot;
    return array;
}

sw_string* sw_expand_home(sw_string* path) {
    if (path == NULL || path->len == 0) {
        return sw_string_from_literal("", 0);
    }
    if (path->data[0] == '~' && (path->len == 1 || sw_is_sep(path->data[1]))) {
        sw_string* home = sw_home_dir();
        if (home->len == 0) {
            return sw_string_from_literal(path->data, path->len);
        }
        if (path->len == 1) {
            return home;
        }
        sw_string* rest = sw_string_from_literal(path->data + 1, path->len - 1);
        return sw_path_normalize(path_join(home, rest));
    }
    return sw_string_from_literal(path->data, path->len);
}

int64_t sw_mkdir_p(sw_string* path) {
    if (path == NULL || path->len == 0) {
        return -1;
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)path->len + 1);
    memcpy(buffer, path->data, (uint64_t)path->len);
    buffer[path->len] = 0;
    int64_t start = 0;
    if (path->len >= 3 && buffer[1] == ':' && sw_is_sep(buffer[2])) {
        start = 3;
    } else if (path->len >= 1 && sw_is_sep(buffer[0])) {
        start = 1;
    }
    for (int64_t i = start; i <= path->len; i++) {
        if (i == path->len || sw_is_sep(buffer[i])) {
            if (i > start) {
                char saved = buffer[i];
                buffer[i] = 0;
                sw_string* prefix = sw_string_from_literal(buffer, i);
                if (sw_mkdir(prefix) != 0 && !is_dir(prefix)) {
                    buffer[i] = saved;
                    return -1;
                }
                buffer[i] = saved;
            }
        }
    }
    return is_dir(path) ? 0 : -1;
}

int64_t sw_disk_free(sw_string* path) {
#if defined(_WIN32)
    extern int GetDiskFreeSpaceExA(
        const char* path,
        uint64_t* free_bytes,
        uint64_t* total_bytes,
        uint64_t* total_free
    );
    uint64_t free_bytes = 0, total_bytes = 0, total_free = 0;
    if (!GetDiskFreeSpaceExA(path->data, &free_bytes, &total_bytes, &total_free)) {
        return -1;
    }
    return (int64_t)free_bytes;
#elif defined(__APPLE__)
    // macOS statvfs：f_bsize@0、f_frsize@8（unsigned long），
    // f_blocks@16/f_bfree@20/f_bavail@24 都是 fsblkcnt_t = uint32（4 字节）。
    extern int statvfs(const char* path, void* buf);
    unsigned char buf[128];
    memset(buf, 0, sizeof(buf));
    if (statvfs(path->data, buf) != 0) {
        return -1;
    }
    uint64_t frsize = *(uint64_t*)(buf + 8);
    uint32_t bavail = *(uint32_t*)(buf + 24);
    return (int64_t)(frsize * (uint64_t)bavail);
#else
    extern int statvfs(const char* path, void* buf);
    unsigned char buf[128];
    memset(buf, 0, sizeof(buf));
    if (statvfs(path->data, buf) != 0) {
        return -1;
    }
    uint64_t frsize = *(uint64_t*)(buf + 8);
    uint64_t bavail = *(uint64_t*)(buf + 32);
    return (int64_t)(frsize * bavail);
#endif
}

int64_t sw_disk_total(sw_string* path) {
#if defined(_WIN32)
    extern int GetDiskFreeSpaceExA(
        const char* path,
        uint64_t* free_bytes,
        uint64_t* total_bytes,
        uint64_t* total_free
    );
    uint64_t free_bytes = 0, total_bytes = 0, total_free = 0;
    if (!GetDiskFreeSpaceExA(path->data, &free_bytes, &total_bytes, &total_free)) {
        return -1;
    }
    return (int64_t)total_bytes;
#elif defined(__APPLE__)
    extern int statvfs(const char* path, void* buf);
    unsigned char buf[128];
    memset(buf, 0, sizeof(buf));
    if (statvfs(path->data, buf) != 0) {
        return -1;
    }
    uint64_t frsize = *(uint64_t*)(buf + 8);
    uint32_t blocks = *(uint32_t*)(buf + 16);
    return (int64_t)(frsize * (uint64_t)blocks);
#else
    extern int statvfs(const char* path, void* buf);
    unsigned char buf[128];
    memset(buf, 0, sizeof(buf));
    if (statvfs(path->data, buf) != 0) {
        return -1;
    }
    uint64_t frsize = *(uint64_t*)(buf + 8);
    uint64_t blocks = *(uint64_t*)(buf + 16);
    return (int64_t)(frsize * blocks);
#endif
}

int64_t sw_is_symlink(sw_string* path) {
#if defined(_WIN32)
    extern unsigned int GetFileAttributesA(const char* path);
    unsigned int attrs = GetFileAttributesA(path->data);
    if (attrs == 0xFFFFFFFFu) {
        return 0;
    }
    return (attrs & 0x400u) ? 1 : 0;
#else
    extern int lstat(const char* path, void* buf);
    unsigned char buf[160];
    memset(buf, 0, sizeof(buf));
    if (lstat(path->data, buf) != 0) {
        return 0;
    }
    unsigned int mode = *(unsigned int*)(buf + SW_STAT_MODE_OFFSET);
    return (mode & 0xF000) == 0xA000 ? 1 : 0;
#endif
}

sw_string* sw_read_symlink(sw_string* path) {
#if defined(_WIN32)
    extern void* CreateFileA(
        const char* path,
        unsigned int access,
        unsigned int share,
        void* sec,
        unsigned int disp,
        unsigned int flags,
        void* templ
    );
    extern int DeviceIoControl(
        void* handle,
        unsigned int code,
        void* in,
        unsigned int in_size,
        void* out,
        unsigned int out_size,
        unsigned int* returned,
        void* overlapped
    );
    extern int CloseHandle(void* handle);
    void* handle = CreateFileA(
        path->data,
        0,
        7,
        NULL,
        3,
        0x02000000u | 0x01000000u,
        NULL
    );
    if (handle == NULL || handle == (void*)-1) {
        return sw_string_from_literal("", 0);
    }
    unsigned char out[16384];
    unsigned int returned = 0;
    if (!DeviceIoControl(handle, 0x000900A8u, NULL, 0, out, sizeof(out), &returned, NULL)) {
        CloseHandle(handle);
        return sw_string_from_literal("", 0);
    }
    CloseHandle(handle);
    unsigned int tag = *(unsigned int*)out;
    int64_t path_base = (tag == 0xA0000003u) ? 16 : 20;
    unsigned int sub_offset = *(unsigned short*)(out + 8);
    unsigned int sub_len = *(unsigned short*)(out + 10);
    if (path_base + sub_offset + sub_len > returned || sub_len < 2) {
        return sw_string_from_literal("", 0);
    }
    const unsigned char* sub = out + path_base + sub_offset;
    int64_t skip = 0;
    if (sub_len >= 8 && sub[0] == '\\' && sub[2] == '?' && sub[4] == '?' && sub[6] == '\\') {
        skip = 8;
    }
    unsigned char wide[8192];
    int64_t units = (sub_len - skip) / 2;
    if (units > 4095) {
        units = 4095;
    }
    memcpy(wide, sub + skip, (uint64_t)units * 2);
    wide[units * 2] = 0;
    wide[units * 2 + 1] = 0;
    sw_string* result = sw_wide_to_utf8(wide);
    return result;
#else
    extern long readlink(const char* path, char* buffer, unsigned long size);
    char buffer[4096];
    long len = readlink(path->data, buffer, sizeof(buffer) - 1);
    if (len < 0) {
        return sw_string_from_literal("", 0);
    }
    buffer[len] = 0;
    return sw_string_from_literal(buffer, len);
#endif
}

int64_t sw_file_mode(sw_string* path) {
#if defined(_WIN32)
    extern unsigned int GetFileAttributesA(const char* path);
    unsigned int attrs = GetFileAttributesA(path->data);
    if (attrs == 0xFFFFFFFFu) {
        return -1;
    }
    return (attrs & 0x1u) ? 292 : 438;  // 0o444 / 0o666
#else
    extern int stat(const char* path, void* buf);
    unsigned char buf[160];
    memset(buf, 0, sizeof(buf));
    if (stat(path->data, buf) != 0) {
        return -1;
    }
    unsigned int mode = *(unsigned int*)(buf + SW_STAT_MODE_OFFSET);
    return (int64_t)(mode & 0x1FFFu);
#endif
}

// ---- string：is_empty / utf8_is_valid / truncate / ellipsis ----

int64_t sw_is_empty(sw_string* text) {
    return text == NULL || text->len == 0 ? 1 : 0;
}

int64_t sw_utf8_is_valid(sw_string* text) {
    if (text == NULL) {
        return 1;
    }
    int64_t i = 0;
    while (i < text->len) {
        unsigned char c = (unsigned char)text->data[i];
        if (c < 0x80) {
            i++;
            continue;
        }
        int64_t need = 0;
        if ((c & 0xE0) == 0xC0) {
            need = 1;
        } else if ((c & 0xF0) == 0xE0) {
            need = 2;
        } else if ((c & 0xF8) == 0xF0) {
            need = 3;
        } else {
            return 0;
        }
        if (i + need >= text->len) {
            return 0;
        }
        for (int64_t k = 1; k <= need; k++) {
            if (((unsigned char)text->data[i + k] & 0xC0) != 0x80) {
                return 0;
            }
        }
        if (need == 1 && c < 0xC2) {
            return 0;
        }
        if (need == 2 && c == 0xE0 && (unsigned char)text->data[i + 1] < 0xA0) {
            return 0;
        }
        if (need == 2 && c == 0xED && (unsigned char)text->data[i + 1] > 0x9F) {
            return 0;
        }
        if (need == 3 && c == 0xF0 && (unsigned char)text->data[i + 1] < 0x90) {
            return 0;
        }
        if (need == 3 && c == 0xF4 && (unsigned char)text->data[i + 1] > 0x8F) {
            return 0;
        }
        if (need == 3 && c > 0xF4) {
            return 0;
        }
        i += need + 1;
    }
    return 1;
}

// ---------------------------------------------------------------------------
// std/hash：字符串哈希（FNV-1a 64 位 / DJB2）
// ---------------------------------------------------------------------------
int64_t fnv1a_64(sw_string* text) {
    uint64_t hash = 14695981039346656037ULL;
    if (text == NULL) {
        return (int64_t)hash;
    }
    for (int64_t i = 0; i < text->len; i++) {
        hash ^= (unsigned char)text->data[i];
        hash *= 1099511628211ULL;
    }
    return (int64_t)hash;
}

int64_t fnv1a_64_seed(sw_string* text, uint64_t seed) {
    uint64_t hash = seed;
    if (text == NULL) {
        return (int64_t)hash;
    }
    for (int64_t i = 0; i < text->len; i++) {
        hash ^= (unsigned char)text->data[i];
        hash *= 1099511628211ULL;
    }
    return (int64_t)hash;
}

int64_t djb2(sw_string* text) {
    uint64_t hash = 5381;
    if (text == NULL) {
        return (int64_t)hash;
    }
    for (int64_t i = 0; i < text->len; i++) {
        hash = ((hash << 5) + hash) + (unsigned char)text->data[i];
    }
    return (int64_t)hash;
}

sw_string* sw_truncate(sw_string* text, int64_t max_chars) {
    if (text == NULL || max_chars <= 0) {
        return sw_string_from_literal("", 0);
    }
    int64_t offset = 0;
    int64_t count = 0;
    while (offset < text->len && count < max_chars) {
        offset += sw_utf8_char_length(text->data, offset, text->len);
        count++;
    }
    return sw_string_from_literal(text->data, offset);
}

sw_string* sw_ellipsis(sw_string* text, int64_t max_chars) {
    if (text == NULL || max_chars <= 0) {
        return sw_string_from_literal("", 0);
    }
    if (utf8_len(text) <= max_chars) {
        return sw_string_from_literal(text->data, text->len);
    }
    if (max_chars <= 3) {
        return sw_string_from_literal("...", 3);
    }
    sw_string* prefix = sw_truncate(text, max_chars - 3);
    char* buffer = (char*)sw_gc_alloc((uint64_t)prefix->len + 4);
    memcpy(buffer, prefix->data, (uint64_t)prefix->len);
    memcpy(buffer + prefix->len, "...", 3);
    buffer[prefix->len + 3] = 0;
    return sw_string_from_literal(buffer, prefix->len + 3);
}

// ---- math：角度转换 / 判定 / tau ----

double sw_deg_to_rad(double degrees) {
    return degrees * 3.14159265358979323846 / 180.0;
}

double sw_rad_to_deg(double radians) {
    return radians * 180.0 / 3.14159265358979323846;
}

int64_t sw_is_nan(double value) {
    return value != value ? 1 : 0;
}

int64_t sw_is_infinite(double value) {
    double inf = 1.0 / 0.0;
    return (value == inf || value == -inf) ? 1 : 0;
}

double sw_tau(void) {
    return 6.28318530717958647692;
}

// ---- time：parse_datetime ----

#define SW_IS_DIGIT(c) ((c) >= '0' && (c) <= '9')

int64_t sw_parse_datetime(sw_string* text) {
    if (text == NULL || text->len < 10) {
        return -1;
    }
    const char* d = text->data;
    if (!(SW_IS_DIGIT(d[0]) && SW_IS_DIGIT(d[1]) && SW_IS_DIGIT(d[2]) && SW_IS_DIGIT(d[3])) ||
        d[4] != '-' || !(SW_IS_DIGIT(d[5]) && SW_IS_DIGIT(d[6])) || d[7] != '-' ||
        !(SW_IS_DIGIT(d[8]) && SW_IS_DIGIT(d[9]))) {
        return -1;
    }
    int64_t year = (d[0] - '0') * 1000 + (d[1] - '0') * 100 + (d[2] - '0') * 10 + (d[3] - '0');
    int64_t month = (d[5] - '0') * 10 + (d[6] - '0');
    int64_t day = (d[8] - '0') * 10 + (d[9] - '0');
    if (month < 1 || month > 12 || day < 1 || day > 31) {
        return -1;
    }
    if (text->len == 10) {
        return sw_time_from_parts(year, month, day, 0, 0, 0);
    }
    if (text->len < 19 || (d[10] != ' ' && d[10] != 'T') ||
        !(SW_IS_DIGIT(d[11]) && SW_IS_DIGIT(d[12])) || d[13] != ':' ||
        !(SW_IS_DIGIT(d[14]) && SW_IS_DIGIT(d[15])) || d[16] != ':' ||
        !(SW_IS_DIGIT(d[17]) && SW_IS_DIGIT(d[18]))) {
        return -1;
    }
    int64_t hour = (d[11] - '0') * 10 + (d[12] - '0');
    int64_t minute = (d[14] - '0') * 10 + (d[15] - '0');
    int64_t second = (d[17] - '0') * 10 + (d[18] - '0');
    if (hour > 23 || minute > 59 || second > 59) {
        return -1;
    }
    return sw_time_from_parts(year, month, day, hour, minute, second);
}

#undef SW_IS_DIGIT

// ---- encoding：base64url / html_escape ----

sw_string* sw_base64url_encode(sw_string* text) {
    sw_string* b64 = sw_base64_encode(text);
    char* buffer = (char*)sw_gc_alloc((uint64_t)b64->len + 1);
    int64_t out = 0;
    for (int64_t i = 0; i < b64->len; i++) {
        char c = b64->data[i];
        if (c == '=') {
            continue;
        }
        buffer[out++] = c == '+' ? '-' : (c == '/' ? '_' : c);
    }
    buffer[out] = 0;
    return sw_string_from_literal(buffer, out);
}

sw_string* sw_base64url_decode(sw_string* text) {
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len + 1);
    int64_t out = 0;
    for (int64_t i = 0; i < text->len; i++) {
        char c = text->data[i];
        buffer[out++] = c == '-' ? '+' : (c == '_' ? '/' : c);
    }
    sw_string* normalized = sw_string_from_literal(buffer, out);
    return sw_base64_decode(normalized);
}

sw_string* sw_html_escape(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len * 6 + 1);
    int64_t out = 0;
    for (int64_t i = 0; i < text->len; i++) {
        char c = text->data[i];
        switch (c) {
            case '&':
                memcpy(buffer + out, "&amp;", 5);
                out += 5;
                break;
            case '<':
                memcpy(buffer + out, "&lt;", 4);
                out += 4;
                break;
            case '>':
                memcpy(buffer + out, "&gt;", 4);
                out += 4;
                break;
            case '"':
                memcpy(buffer + out, "&quot;", 6);
                out += 6;
                break;
            case '\'':
                memcpy(buffer + out, "&#39;", 5);
                out += 5;
                break;
            default:
                buffer[out++] = c;
        }
    }
    buffer[out] = 0;
    return sw_string_from_literal(buffer, out);
}

// HTML 反转义：&amp; &lt; &gt; &quot; &#39; 以及数字实体 &#NN; / &#xHH;。
sw_string* sw_html_unescape(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len + 1);
    int64_t out = 0;
    int64_t i = 0;
    while (i < text->len) {
        if (text->data[i] == '&') {
            // 找结尾 ';'
            int64_t end = i + 1;
            while (end < text->len && text->data[end] != ';' && end - i <= 32) {
                end++;
            }
            if (end < text->len && text->data[end] == ';') {
                int64_t ent_len = end - i - 1;
                if (ent_len == 3 && memcmp(text->data + i + 1, "amp", 3) == 0) {
                    buffer[out++] = '&';
                    i = end + 1;
                    continue;
                }
                if (ent_len == 2 && memcmp(text->data + i + 1, "lt", 2) == 0) {
                    buffer[out++] = '<';
                    i = end + 1;
                    continue;
                }
                if (ent_len == 2 && memcmp(text->data + i + 1, "gt", 2) == 0) {
                    buffer[out++] = '>';
                    i = end + 1;
                    continue;
                }
                if (ent_len == 4 && memcmp(text->data + i + 1, "quot", 4) == 0) {
                    buffer[out++] = '"';
                    i = end + 1;
                    continue;
                }
                if (ent_len == 4 && memcmp(text->data + i + 1, "#39;", 4) != 0 &&
                    memcmp(text->data + i + 1, "apos", 4) == 0) {
                    buffer[out++] = '\'';
                    i = end + 1;
                    continue;
                }
                if (ent_len == 4 && memcmp(text->data + i + 1, "#39", 3) == 0) {
                    buffer[out++] = '\'';
                    i = end + 1;
                    continue;
                }
                // 数字实体 &#123; 或 &#x1F;
                if (ent_len >= 2 && text->data[i + 1] == '#') {
                    int64_t code = 0;
                    int64_t k = i + 2;
                    int hex_mode = 0;
                    if (k < end && (text->data[k] == 'x' || text->data[k] == 'X')) {
                        hex_mode = 1;
                        k++;
                    }
                    int64_t digits = 0;
                    int valid = 1;
                    while (k < end) {
                        char c = text->data[k];
                        int digit = -1;
                        if (c >= '0' && c <= '9') {
                            digit = c - '0';
                        } else if (hex_mode && c >= 'a' && c <= 'f') {
                            digit = c - 'a' + 10;
                        } else if (hex_mode && c >= 'A' && c <= 'F') {
                            digit = c - 'A' + 10;
                        } else {
                            valid = 0;
                            break;
                        }
                        code = code * (hex_mode ? 16 : 10) + digit;
                        digits++;
                        k++;
                    }
                    if (valid && digits > 0 && code >= 0 && code <= 0x10FFFF) {
                        // 码点转 UTF-8。
                        if (code < 0x80) {
                            buffer[out++] = (char)code;
                        } else if (code < 0x800) {
                            buffer[out++] = (char)(0xC0 | (code >> 6));
                            buffer[out++] = (char)(0x80 | (code & 0x3F));
                        } else if (code < 0x10000) {
                            buffer[out++] = (char)(0xE0 | (code >> 12));
                            buffer[out++] = (char)(0x80 | ((code >> 6) & 0x3F));
                            buffer[out++] = (char)(0x80 | (code & 0x3F));
                        } else {
                            buffer[out++] = (char)(0xF0 | (code >> 18));
                            buffer[out++] = (char)(0x80 | ((code >> 12) & 0x3F));
                            buffer[out++] = (char)(0x80 | ((code >> 6) & 0x3F));
                            buffer[out++] = (char)(0x80 | (code & 0x3F));
                        }
                        i = end + 1;
                        continue;
                    }
                }
            }
            // 未识别的实体按字面复制 '&'。
        }
        buffer[out++] = text->data[i++];
    }
    buffer[out] = 0;
    return sw_string_from_literal(buffer, out);
}

// ---- json：stringify / object_keys / type_name ----

typedef struct sw_str_builder {
    char* data;
    int64_t len;
    int64_t cap;
} sw_str_builder;

static void sw_builder_grow(sw_str_builder* builder, int64_t extra) {
    if (builder->len + extra + 1 <= builder->cap) {
        return;
    }
    int64_t new_cap = builder->cap * 2 + extra + 64;
    builder->data = (char*)realloc(builder->data, (sw_size)new_cap);
    builder->cap = new_cap;
}

static void sw_builder_append(sw_str_builder* builder, const char* text, int64_t len) {
    sw_builder_grow(builder, len);
    memcpy(builder->data + builder->len, text, (uint64_t)len);
    builder->len += len;
}

static void sw_builder_char(sw_str_builder* builder, char c) {
    sw_builder_grow(builder, 1);
    builder->data[builder->len++] = c;
}

static void sw_json_escape_append(sw_str_builder* builder, const char* text, int64_t len) {
    static const char* hex = "0123456789abcdef";
    sw_builder_char(builder, '"');
    for (int64_t i = 0; i < len; i++) {
        unsigned char c = (unsigned char)text[i];
        switch (c) {
            case '"':
                sw_builder_append(builder, "\\\"", 2);
                break;
            case '\\':
                sw_builder_append(builder, "\\\\", 2);
                break;
            case '\n':
                sw_builder_append(builder, "\\n", 2);
                break;
            case '\r':
                sw_builder_append(builder, "\\r", 2);
                break;
            case '\t':
                sw_builder_append(builder, "\\t", 2);
                break;
            default:
                if (c < 0x20) {
                    char escape[6] = {'\\', 'u', '0', '0', hex[(c >> 4) & 0xF], hex[c & 0xF]};
                    sw_builder_append(builder, escape, 6);
                } else {
                    sw_builder_char(builder, (char)c);
                }
        }
    }
    sw_builder_char(builder, '"');
}

static void sw_json_write(sw_str_builder* builder, sw_json* value) {
    switch (value->kind) {
        case 0:
            sw_builder_append(builder, "null", 4);
            break;
        case 1:
            sw_builder_append(builder, value->int_value ? "true" : "false", value->int_value ? 4 : 5);
            break;
        case 2: {
            char number[32];
            int len = snprintf(number, sizeof(number), "%lld", (long long)value->int_value);
            sw_builder_append(builder, number, len);
            break;
        }
        case 3: {
            char number[64];
            int len = snprintf(number, sizeof(number), "%.17g", value->float_value);
            sw_builder_append(builder, number, len);
            break;
        }
        case 4:
            sw_json_escape_append(builder, value->string_value, value->length);
            break;
        case 5:
            sw_builder_char(builder, '[');
            for (int64_t i = 0; i < value->length; i++) {
                if (i > 0) {
                    sw_builder_char(builder, ',');
                }
                sw_json_write(builder, value->items[i]);
            }
            sw_builder_char(builder, ']');
            break;
        case 6:
            sw_builder_char(builder, '{');
            for (int64_t i = 0; i < value->length; i++) {
                if (i > 0) {
                    sw_builder_char(builder, ',');
                }
                sw_json_escape_append(builder, value->keys[i], (int64_t)strlen(value->keys[i]));
                sw_builder_char(builder, ':');
                sw_json_write(builder, value->items[i]);
            }
            sw_builder_char(builder, '}');
            break;
        default:
            sw_builder_append(builder, "null", 4);
    }
}

sw_string* json_stringify(void* value) {
    sw_json* node = (sw_json*)value;
    if (node == NULL) {
        return sw_string_from_literal("null", 4);
    }
    sw_str_builder builder = {NULL, 0, 0};
    sw_json_write(&builder, node);
    sw_builder_grow(&builder, 1);
    builder.data[builder.len] = 0;
    sw_string* result = sw_string_from_literal(builder.data, builder.len);
    free(builder.data);
    return result;
}

sw_array* json_object_keys(void* value) {
    sw_json* node = (sw_json*)value;
    sw_array* array = sw_array_new(8, node != NULL && node->kind == 6 ? node->length : 0);
    if (node == NULL || node->kind != 6) {
        return array;
    }
    for (int64_t i = 0; i < node->length; i++) {
        ((int64_t*)array->data)[i] =
            (int64_t)sw_string_from_literal(node->keys[i], (int64_t)strlen(node->keys[i]));
    }
    return array;
}

sw_string* json_type_name(void* value) {
    sw_json* node = (sw_json*)value;
    int64_t kind = node == NULL ? 0 : node->kind;
    switch (kind) {
        case 0:
            return sw_string_from_literal("null", 4);
        case 1:
            return sw_string_from_literal("bool", 4);
        case 2:
            return sw_string_from_literal("int", 3);
        case 3:
            return sw_string_from_literal("float", 5);
        case 4:
            return sw_string_from_literal("string", 6);
        case 5:
            return sw_string_from_literal("array", 5);
        default:
            return sw_string_from_literal("object", 6);
    }
}

// ---------------------------------------------------------------------------
// 数组工具：类型化排序/反转/最值/求和/去重（std/array）
// ---------------------------------------------------------------------------

static int sw_cmp_i64(const void* a, const void* b) {
    int64_t x = *(const int64_t*)a;
    int64_t y = *(const int64_t*)b;
    return x < y ? -1 : (x > y ? 1 : 0);
}

static int sw_cmp_f64(const void* a, const void* b) {
    double x = *(const double*)a;
    double y = *(const double*)b;
    return x < y ? -1 : (x > y ? 1 : 0);
}

static int sw_cmp_str(const void* a, const void* b) {
    sw_string* x = *(sw_string* const*)a;
    sw_string* y = *(sw_string* const*)b;
    int64_t min = x->len < y->len ? x->len : y->len;
    int cmp = min > 0 ? memcmp(x->data, y->data, (uint64_t)min) : 0;
    if (cmp != 0) {
        return cmp;
    }
    return x->len < y->len ? -1 : (x->len > y->len ? 1 : 0);
}

void sw_sort_int(sw_array* items) {
    if (items != NULL && items->len > 1) {
        qsort(items->data, (uint64_t)items->len, 8, sw_cmp_i64);
    }
}

void sw_sort_float(sw_array* items) {
    if (items != NULL && items->len > 1) {
        qsort(items->data, (uint64_t)items->len, 8, sw_cmp_f64);
    }
}

void sw_sort_string(sw_array* items) {
    if (items != NULL && items->len > 1) {
        qsort(items->data, (uint64_t)items->len, 8, sw_cmp_str);
    }
}

static int sw_cmp_i64_desc(const void* a, const void* b) {
    int64_t x = *(const int64_t*)a;
    int64_t y = *(const int64_t*)b;
    return x > y ? -1 : (x < y ? 1 : 0);
}

static int sw_cmp_f64_desc(const void* a, const void* b) {
    double x = *(const double*)a;
    double y = *(const double*)b;
    return x > y ? -1 : (x < y ? 1 : 0);
}

static int sw_cmp_str_desc(const void* a, const void* b) {
    sw_string* x = *(sw_string* const*)a;
    sw_string* y = *(sw_string* const*)b;
    int64_t min = x->len < y->len ? x->len : y->len;
    int cmp = min > 0 ? memcmp(x->data, y->data, (uint64_t)min) : 0;
    if (cmp != 0) {
        return -cmp;
    }
    return x->len < y->len ? 1 : (x->len > y->len ? -1 : 0);
}

void sw_sort_int_desc(sw_array* items) {
    if (items != NULL && items->len > 1) {
        qsort(items->data, (uint64_t)items->len, 8, sw_cmp_i64_desc);
    }
}

void sw_sort_float_desc(sw_array* items) {
    if (items != NULL && items->len > 1) {
        qsort(items->data, (uint64_t)items->len, 8, sw_cmp_f64_desc);
    }
}

void sw_sort_string_desc(sw_array* items) {
    if (items != NULL && items->len > 1) {
        qsort(items->data, (uint64_t)items->len, 8, sw_cmp_str_desc);
    }
}

void sw_reverse_int(sw_array* items) {
    if (items == NULL) {
        return;
    }
    int64_t* data = (int64_t*)items->data;
    for (int64_t i = 0; i < items->len / 2; i++) {
        int64_t tmp = data[i];
        data[i] = data[items->len - 1 - i];
        data[items->len - 1 - i] = tmp;
    }
}

void sw_reverse_float(sw_array* items) {
    if (items == NULL) {
        return;
    }
    double* data = (double*)items->data;
    for (int64_t i = 0; i < items->len / 2; i++) {
        double tmp = data[i];
        data[i] = data[items->len - 1 - i];
        data[items->len - 1 - i] = tmp;
    }
}

void sw_reverse_string(sw_array* items) {
    if (items == NULL) {
        return;
    }
    int64_t* data = (int64_t*)items->data;
    for (int64_t i = 0; i < items->len / 2; i++) {
        int64_t tmp = data[i];
        data[i] = data[items->len - 1 - i];
        data[items->len - 1 - i] = tmp;
    }
}

int64_t sw_min_int(sw_array* items) {
    if (items == NULL || items->len == 0) {
        return 0;
    }
    int64_t* data = (int64_t*)items->data;
    int64_t result = data[0];
    for (int64_t i = 1; i < items->len; i++) {
        if (data[i] < result) {
            result = data[i];
        }
    }
    return result;
}

int64_t sw_max_int(sw_array* items) {
    if (items == NULL || items->len == 0) {
        return 0;
    }
    int64_t* data = (int64_t*)items->data;
    int64_t result = data[0];
    for (int64_t i = 1; i < items->len; i++) {
        if (data[i] > result) {
            result = data[i];
        }
    }
    return result;
}

int64_t sw_sum_int(sw_array* items) {
    if (items == NULL) {
        return 0;
    }
    int64_t* data = (int64_t*)items->data;
    int64_t total = 0;
    for (int64_t i = 0; i < items->len; i++) {
        total += data[i];
    }
    return total;
}

double sw_min_float(sw_array* items) {
    if (items == NULL || items->len == 0) {
        return 0.0;
    }
    double* data = (double*)items->data;
    double result = data[0];
    for (int64_t i = 1; i < items->len; i++) {
        if (data[i] < result) {
            result = data[i];
        }
    }
    return result;
}

double sw_max_float(sw_array* items) {
    if (items == NULL || items->len == 0) {
        return 0.0;
    }
    double* data = (double*)items->data;
    double result = data[0];
    for (int64_t i = 1; i < items->len; i++) {
        if (data[i] > result) {
            result = data[i];
        }
    }
    return result;
}

double sw_sum_float(sw_array* items) {
    if (items == NULL) {
        return 0.0;
    }
    double* data = (double*)items->data;
    double total = 0.0;
    for (int64_t i = 0; i < items->len; i++) {
        total += data[i];
    }
    return total;
}

sw_array* sw_unique_string(sw_array* items) {
    if (items == NULL) {
        return sw_array_new(8, 0);
    }
    sw_array* result = sw_array_new(8, items->len);
    int64_t slot = 0;
    for (int64_t i = 0; i < items->len; i++) {
        sw_string* item = (sw_string*)((int64_t*)items->data)[i];
        int64_t seen = 0;
        for (int64_t k = 0; k < slot; k++) {
            if (string_eq(item, (sw_string*)((int64_t*)result->data)[k])) {
                seen = 1;
                break;
            }
        }
        if (!seen) {
            ((int64_t*)result->data)[slot++] = (int64_t)item;
        }
    }
    result->len = slot;
    result->cap = slot;
    return result;
}

// ---------------------------------------------------------------------------
// array 实用：concat / insert / remove_at / unique / 极值位置（返回新数组，
// 不修改原数组；elem_size 1=u8、其余 8 字节槽）。
// ---------------------------------------------------------------------------

static sw_array* sw_array_concat_impl(sw_array* a, sw_array* b, int64_t elem_size) {
    int64_t alen = a != NULL ? a->len : 0;
    int64_t blen = b != NULL ? b->len : 0;
    sw_array* result = sw_array_new(elem_size, alen + blen);
    if (alen > 0) {
        memcpy(result->data, a->data, (sw_size)((uint64_t)alen * elem_size));
    }
    if (blen > 0) {
        memcpy(
            (char*)result->data + (uintptr_t)(alen * elem_size),
            b->data,
            (sw_size)((uint64_t)blen * elem_size)
        );
    }
    return result;
}

sw_array* sw_array_concat_int(sw_array* a, sw_array* b) {
    return sw_array_concat_impl(a, b, 8);
}

sw_array* sw_array_concat_float(sw_array* a, sw_array* b) {
    return sw_array_concat_impl(a, b, 8);
}

sw_array* sw_array_concat_str(sw_array* a, sw_array* b) {
    return sw_array_concat_impl(a, b, 8);
}

// 在 index 处插入元素（index 越界钳制到 [0, len]），返回新数组。
static sw_array* sw_array_insert_impl(sw_array* items, int64_t index, int64_t value, int64_t elem_size) {
    int64_t len = items != NULL ? items->len : 0;
    if (index < 0) {
        index = 0;
    }
    if (index > len) {
        index = len;
    }
    sw_array* result = sw_array_new(elem_size, len + 1);
    if (index > 0) {
        memcpy(result->data, items->data, (sw_size)((uint64_t)index * elem_size));
    }
    if (elem_size == 1) {
        ((unsigned char*)result->data)[index] = (unsigned char)value;
    } else {
        ((int64_t*)result->data)[index] = value;
    }
    if (len > index) {
        memcpy(
            (char*)result->data + (uintptr_t)((index + 1) * elem_size),
            (char*)items->data + (uintptr_t)(index * elem_size),
            (sw_size)((uint64_t)(len - index) * elem_size)
        );
    }
    return result;
}

sw_array* sw_array_insert_int(sw_array* items, int64_t index, int64_t value) {
    return sw_array_insert_impl(items, index, value, 8);
}

sw_array* sw_array_insert_float(sw_array* items, int64_t index, double value) {
    int64_t bits = 0;
    memcpy(&bits, &value, 8);
    return sw_array_insert_impl(items, index, bits, 8);
}

sw_array* sw_array_insert_str(sw_array* items, int64_t index, sw_string* value) {
    return sw_array_insert_impl(items, index, (int64_t)value, 8);
}

// 删除 index 处元素，返回新数组；越界返回原样复制。
static sw_array* sw_array_remove_at_impl(sw_array* items, int64_t index, int64_t elem_size) {
    int64_t len = items != NULL ? items->len : 0;
    if (len == 0) {
        return sw_array_new(elem_size, 0);
    }
    if (index < 0 || index >= len) {
        return sw_array_concat_impl(items, NULL, elem_size);
    }
    sw_array* result = sw_array_new(elem_size, len - 1);
    if (index > 0) {
        memcpy(result->data, items->data, (sw_size)((uint64_t)index * elem_size));
    }
    if (len - index - 1 > 0) {
        memcpy(
            (char*)result->data + (uintptr_t)(index * elem_size),
            (char*)items->data + (uintptr_t)((index + 1) * elem_size),
            (sw_size)((uint64_t)(len - index - 1) * elem_size)
        );
    }
    return result;
}

sw_array* sw_array_remove_at_int(sw_array* items, int64_t index) {
    return sw_array_remove_at_impl(items, index, 8);
}

sw_array* sw_array_remove_at_float(sw_array* items, int64_t index) {
    return sw_array_remove_at_impl(items, index, 8);
}

sw_array* sw_array_remove_at_str(sw_array* items, int64_t index) {
    return sw_array_remove_at_impl(items, index, 8);
}

// 去重（保持首次出现顺序），返回新数组。
sw_array* sw_array_unique_int(sw_array* items) {
    if (items == NULL) {
        return sw_array_new(8, 0);
    }
    sw_array* result = sw_array_new(8, items->len);
    int64_t slot = 0;
    int64_t* data = (int64_t*)items->data;
    for (int64_t i = 0; i < items->len; i++) {
        int64_t seen = 0;
        for (int64_t k = 0; k < slot; k++) {
            if (((int64_t*)result->data)[k] == data[i]) {
                seen = 1;
                break;
            }
        }
        if (!seen) {
            ((int64_t*)result->data)[slot++] = data[i];
        }
    }
    result->len = slot;
    result->cap = slot;
    return result;
}

sw_array* sw_array_unique_float(sw_array* items) {
    if (items == NULL) {
        return sw_array_new(8, 0);
    }
    sw_array* result = sw_array_new(8, items->len);
    int64_t slot = 0;
    double* data = (double*)items->data;
    for (int64_t i = 0; i < items->len; i++) {
        int64_t seen = 0;
        for (int64_t k = 0; k < slot; k++) {
            if (((double*)result->data)[k] == data[i]) {
                seen = 1;
                break;
            }
        }
        if (!seen) {
            ((double*)result->data)[slot++] = data[i];
        }
    }
    result->len = slot;
    result->cap = slot;
    return result;
}

int64_t sw_min_index_float(sw_array* items) {
    if (items == NULL || items->len == 0) {
        return -1;
    }
    double* data = (double*)items->data;
    int64_t best = 0;
    for (int64_t i = 1; i < items->len; i++) {
        if (data[i] < data[best]) {
            best = i;
        }
    }
    return best;
}

int64_t sw_max_index_float(sw_array* items) {
    if (items == NULL || items->len == 0) {
        return -1;
    }
    double* data = (double*)items->data;
    int64_t best = 0;
    for (int64_t i = 1; i < items->len; i++) {
        if (data[i] > data[best]) {
            best = i;
        }
    }
    return best;
}

// ---------------------------------------------------------------------------
// map：字符串键字典（GC 管理，保持插入顺序）
// ---------------------------------------------------------------------------

typedef struct sw_map_node {
    struct sw_map_node* next;
    sw_string* key;
    int64_t tag;
    int64_t value;  // string 时为指针，int 为数值，float 为位模式，bool 为 0/1
} sw_map_node;

typedef struct sw_map {
    sw_map_node* head;
    sw_map_node* tail;
    int64_t count;
} sw_map;

void* sw_map_new(void) {
    sw_map* map = (sw_map*)sw_gc_alloc(sizeof(sw_map));
    map->head = NULL;
    map->tail = NULL;
    map->count = 0;
    return map;
}

static sw_map_node* sw_map_find(sw_map* map, sw_string* key) {
    for (sw_map_node* node = map->head; node != NULL; node = node->next) {
        if (string_eq(node->key, key)) {
            return node;
        }
    }
    return NULL;
}

int64_t sw_map_set(void* handle, sw_string* key, sw_string* value) {
    sw_map* map = (sw_map*)handle;
    if (map == NULL || key == NULL) {
        return -1;
    }
    sw_map_node* node = sw_map_find(map, key);
    if (node != NULL) {
        node->tag = SW_TAG_STR;
        node->value = (int64_t)value;
        return 0;
    }
    sw_map_node* fresh = (sw_map_node*)sw_gc_alloc(sizeof(sw_map_node));
    fresh->next = NULL;
    fresh->key = key;
    fresh->tag = SW_TAG_STR;
    fresh->value = (int64_t)value;
    if (map->tail != NULL) {
        map->tail->next = fresh;
    } else {
        map->head = fresh;
    }
    map->tail = fresh;
    map->count++;
    return 0;
}

sw_string* sw_map_get(void* handle, sw_string* key) {
    sw_map* map = (sw_map*)handle;
    if (map == NULL) {
        return NULL;
    }
    sw_map_node* node = sw_map_find(map, key);
    return node != NULL && node->tag == SW_TAG_STR ? (sw_string*)node->value : NULL;
}

int64_t sw_map_set_int(void* handle, sw_string* key, int64_t value) {
    sw_map* map = (sw_map*)handle;
    if (map == NULL || key == NULL) {
        return -1;
    }
    sw_map_node* node = sw_map_find(map, key);
    if (node != NULL) {
        node->tag = SW_TAG_INT;
        node->value = value;
        return 0;
    }
    sw_map_node* fresh = (sw_map_node*)sw_gc_alloc(sizeof(sw_map_node));
    fresh->next = NULL;
    fresh->key = key;
    fresh->tag = SW_TAG_INT;
    fresh->value = value;
    if (map->tail != NULL) {
        map->tail->next = fresh;
    } else {
        map->head = fresh;
    }
    map->tail = fresh;
    map->count++;
    return 0;
}

// 读取 int 值；键不存在或类型不符返回 fallback。
int64_t sw_map_get_int(void* handle, sw_string* key, int64_t fallback) {
    sw_map* map = (sw_map*)handle;
    if (map == NULL) {
        return fallback;
    }
    sw_map_node* node = sw_map_find(map, key);
    return node != NULL && node->tag == SW_TAG_INT ? node->value : fallback;
}

// 计数累加：键存在且为 int 则加 delta，否则以 delta 初始化；返回新值。
int64_t sw_map_inc(void* handle, sw_string* key, int64_t delta) {
    sw_map* map = (sw_map*)handle;
    if (map == NULL || key == NULL) {
        return 0;
    }
    sw_map_node* node = sw_map_find(map, key);
    if (node != NULL && node->tag == SW_TAG_INT) {
        node->value += delta;
        return node->value;
    }
    if (node != NULL) {
        node->tag = SW_TAG_INT;
        node->value = delta;
        return delta;
    }
    sw_map_set_int(handle, key, delta);
    return delta;
}

int64_t sw_map_set_float(void* handle, sw_string* key, double value) {
    sw_map* map = (sw_map*)handle;
    if (map == NULL || key == NULL) {
        return -1;
    }
    int64_t bits = 0;
    memcpy(&bits, &value, 8);
    sw_map_node* node = sw_map_find(map, key);
    if (node != NULL) {
        node->tag = SW_TAG_FLOAT;
        node->value = bits;
        return 0;
    }
    sw_map_node* fresh = (sw_map_node*)sw_gc_alloc(sizeof(sw_map_node));
    fresh->next = NULL;
    fresh->key = key;
    fresh->tag = SW_TAG_FLOAT;
    fresh->value = bits;
    if (map->tail != NULL) {
        map->tail->next = fresh;
    } else {
        map->head = fresh;
    }
    map->tail = fresh;
    map->count++;
    return 0;
}

double sw_map_get_float(void* handle, sw_string* key, double fallback) {
    sw_map* map = (sw_map*)handle;
    if (map == NULL) {
        return fallback;
    }
    sw_map_node* node = sw_map_find(map, key);
    if (node != NULL && node->tag == SW_TAG_FLOAT) {
        double value;
        memcpy(&value, &node->value, 8);
        return value;
    }
    return fallback;
}

int64_t sw_map_set_bool(void* handle, sw_string* key, int64_t value) {
    sw_map* map = (sw_map*)handle;
    if (map == NULL || key == NULL) {
        return -1;
    }
    sw_map_node* node = sw_map_find(map, key);
    if (node != NULL) {
        node->tag = SW_TAG_BOOL;
        node->value = value ? 1 : 0;
        return 0;
    }
    sw_map_node* fresh = (sw_map_node*)sw_gc_alloc(sizeof(sw_map_node));
    fresh->next = NULL;
    fresh->key = key;
    fresh->tag = SW_TAG_BOOL;
    fresh->value = value ? 1 : 0;
    if (map->tail != NULL) {
        map->tail->next = fresh;
    } else {
        map->head = fresh;
    }
    map->tail = fresh;
    map->count++;
    return 0;
}

int64_t sw_map_get_bool(void* handle, sw_string* key, int64_t fallback) {
    sw_map* map = (sw_map*)handle;
    if (map == NULL) {
        return fallback;
    }
    sw_map_node* node = sw_map_find(map, key);
    return node != NULL && node->tag == SW_TAG_BOOL ? node->value : fallback;
}

int64_t sw_map_has(void* handle, sw_string* key) {
    sw_map* map = (sw_map*)handle;
    return map != NULL && sw_map_find(map, key) != NULL ? 1 : 0;
}

int64_t sw_map_remove(void* handle, sw_string* key) {
    sw_map* map = (sw_map*)handle;
    if (map == NULL) {
        return -1;
    }
    sw_map_node* prev = NULL;
    sw_map_node* node = map->head;
    while (node != NULL) {
        if (string_eq(node->key, key)) {
            if (prev != NULL) {
                prev->next = node->next;
            } else {
                map->head = node->next;
            }
            if (map->tail == node) {
                map->tail = prev;
            }
            map->count--;
            return 0;
        }
        prev = node;
        node = node->next;
    }
    return -1;
}

int64_t sw_map_len(void* handle) {
    sw_map* map = (sw_map*)handle;
    return map != NULL ? map->count : 0;
}

int64_t sw_map_clear(void* handle) {
    sw_map* map = (sw_map*)handle;
    if (map == NULL) {
        return -1;
    }
    map->head = NULL;
    map->tail = NULL;
    map->count = 0;
    // 节点由 GC 管理，出链后下次回收自动释放。
    return 0;
}

sw_array* sw_map_keys(void* handle) {
    sw_map* map = (sw_map*)handle;
    sw_array* array = sw_array_new(8, map != NULL ? map->count : 0);
    if (map == NULL) {
        return array;
    }
    int64_t slot = 0;
    for (sw_map_node* node = map->head; node != NULL; node = node->next) {
        ((int64_t*)array->data)[slot++] = (int64_t)node->key;
    }
    return array;
}

sw_array* sw_map_values(void* handle) {
    sw_map* map = (sw_map*)handle;
    sw_array* array = sw_array_new(8, map != NULL ? map->count : 0);
    if (map == NULL) {
        return array;
    }
    int64_t slot = 0;
    for (sw_map_node* node = map->head; node != NULL; node = node->next) {
        // 值数组：string 值存指针；其余类型存数值（与 map_values 语义一致）。
        ((int64_t*)array->data)[slot++] = node->value;
    }
    return array;
}

// ---------------------------------------------------------------------------
// 进程增强（std/os）：环境变量 / 工作目录 / 分离 stdout+stderr / 存活检测。
// 注意：run_stdout_stderr 顺序读取两管道，任一输出超过管道缓冲（Windows
// 4KB / POSIX 64KB）时可能阻塞——建议用于小输出。
// ---------------------------------------------------------------------------

#if defined(_WIN32)

// 把 map 构造成 CreateProcessA 环境块（"K=V\0K=V\0\0"；仅 string 值）。
static char* sw_build_env_block(sw_map* map) {
    if (map == NULL || map->count == 0) {
        return NULL;
    }
    int64_t total = 1;
    for (sw_map_node* node = map->head; node != NULL; node = node->next) {
        if (node->tag != SW_TAG_STR) {
            continue;
        }
        sw_string* key = node->key;
        sw_string* value = (sw_string*)node->value;
        total += key->len + 1 + value->len + 1;
    }
    char* block = (char*)sw_gc_alloc((uint64_t)total);
    int64_t out = 0;
    for (sw_map_node* node = map->head; node != NULL; node = node->next) {
        if (node->tag != SW_TAG_STR) {
            continue;
        }
        sw_string* key = node->key;
        sw_string* value = (sw_string*)node->value;
        memcpy(block + out, key->data, (sw_size)key->len);
        out += key->len;
        block[out++] = '=';
        memcpy(block + out, value->data, (sw_size)value->len);
        out += value->len;
        block[out++] = 0;
    }
    block[out] = 0;
    return block;
}

// 读取管道到 EOF，返回 sw_string。
static sw_string* sw_read_pipe_all(void* read_handle) {
    char chunk[4096];
    char* buffer = (char*)malloc(4096);
    int64_t capacity = 4096;
    int64_t length = 0;
    while (1) {
        unsigned int got = 0;
        if (!ReadFile(read_handle, chunk, sizeof(chunk), &got, NULL) || got == 0) {
            break;
        }
        if (length + (int64_t)got > capacity) {
            capacity = (length + (int64_t)got) * 2;
            buffer = (char*)realloc(buffer, (sw_size)capacity);
        }
        memcpy(buffer + length, chunk, got);
        length += (int64_t)got;
    }
    sw_string* result = sw_string_from_literal(buffer, length);
    free(buffer);
    return result;
}

// 通用启动：env_block/cwd 可为 NULL；err_mode 1 时 stderr 走单独管道。
static sw_string* sw_run_impl_env(
    sw_string* cmd, sw_array* args, char* env_block, sw_string* dir,
    void** err_read_out
) {
    void* out_read = NULL;
    void* out_write = NULL;
    void* err_read = NULL;
    void* err_write = NULL;
    if (!CreatePipe(&out_read, &out_write, NULL, 0)) {
        return sw_string_from_literal("", 0);
    }
    SetHandleInformation(out_write, 1, 1);
    if (err_read_out != NULL) {
        if (!CreatePipe(&err_read, &err_write, NULL, 0)) {
            CloseHandle(out_read);
            CloseHandle(out_write);
            return sw_string_from_literal("", 0);
        }
        SetHandleInformation(err_write, 1, 1);
    }
    char* cmdline = sw_build_cmdline(cmd, args);
    sw_startup_info startup;
    memset(&startup, 0, sizeof(startup));
    startup.cb = sizeof(startup);
    startup.flags = 0x00000100u;  // STARTF_USESTDHANDLES
    startup.h_std_output = out_write;
    startup.h_std_error = err_write != NULL ? err_write : out_write;
    sw_proc_info info;
    memset(&info, 0, sizeof(info));
    int ok = CreateProcessA(
        NULL, cmdline, NULL, NULL, 1, 0, env_block,
        dir != NULL ? dir->data : NULL, &startup, &info
    );
    CloseHandle(out_write);
    if (err_write != NULL) {
        CloseHandle(err_write);
    }
    if (!ok) {
        CloseHandle(out_read);
        if (err_read != NULL) {
            CloseHandle(err_read);
        }
        return sw_string_from_literal("", 0);
    }
    CloseHandle(info.h_thread);
    sw_string* result = sw_read_pipe_all(out_read);
    CloseHandle(out_read);
    if (err_read_out != NULL) {
        *err_read_out = err_read;
    }
    WaitForSingleObject(info.h_process, 0xFFFFFFFFu);
    CloseHandle(info.h_process);
    return result;
}

sw_string* sw_run_with_env(sw_string* cmd, sw_array* args, void* env_handle) {
    return sw_run_impl_env(cmd, args, sw_build_env_block((sw_map*)env_handle), NULL, NULL);
}

sw_string* sw_run_in_dir(sw_string* cmd, sw_array* args, sw_string* dir) {
    return sw_run_impl_env(cmd, args, NULL, dir, NULL);
}

sw_array* sw_run_stdout_stderr(sw_string* cmd, sw_array* args) {
    void* err_read = NULL;
    sw_string* out = sw_run_impl_env(cmd, args, NULL, NULL, &err_read);
    sw_string* err = err_read != NULL ? sw_read_pipe_all(err_read) : sw_string_from_literal("", 0);
    if (err_read != NULL) {
        CloseHandle(err_read);
    }
    sw_array* result = sw_array_new(8, 2);
    ((int64_t*)result->data)[0] = (int64_t)out;
    ((int64_t*)result->data)[1] = (int64_t)err;
    return result;
}

int64_t sw_is_process_running(int64_t pid) {
    extern void* OpenProcess(unsigned int access, int inherit, unsigned int pid);
    extern int GetExitCodeProcess(void* handle, unsigned int* code);
    extern int CloseHandle(void* handle);
    void* handle = OpenProcess(0x1000u /*PROCESS_QUERY_LIMITED_INFORMATION*/, 0, (unsigned int)pid);
    if (handle == NULL) {
        return 0;
    }
    unsigned int code = 0;
    GetExitCodeProcess(handle, &code);
    CloseHandle(handle);
    return code == 259u /*STILL_ACTIVE*/ ? 1 : 0;
}

#else  // POSIX（Linux / macOS）

// 构建 exec 用 envp（"K=V" 数组，NULL 结尾；仅 string 值）。
static char** sw_build_envp(sw_map* map) {
    if (map == NULL || map->count == 0) {
        return NULL;
    }
    char** envp = (char**)sw_gc_alloc((uint64_t)(map->count + 1) * sizeof(char*));
    int64_t slot = 0;
    for (sw_map_node* node = map->head; node != NULL; node = node->next) {
        if (node->tag != SW_TAG_STR) {
            continue;
        }
        sw_string* key = node->key;
        sw_string* value = (sw_string*)node->value;
        char* entry = (char*)sw_gc_alloc((uint64_t)key->len + 1 + value->len + 1);
        memcpy(entry, key->data, (sw_size)key->len);
        entry[key->len] = '=';
        memcpy(entry + key->len + 1, value->data, (sw_size)value->len);
        entry[key->len + 1 + value->len] = 0;
        envp[slot++] = entry;
    }
    envp[slot] = NULL;
    return envp;
}

static sw_string* sw_read_fd_all(int fd) {
    char chunk[4096];
    char* buffer = (char*)malloc(4096);
    int64_t capacity = 4096;
    int64_t length = 0;
    while (1) {
        long got = read(fd, chunk, sizeof(chunk));
        if (got <= 0) {
            break;
        }
        if (length + got > capacity) {
            capacity = (length + got) * 2;
            buffer = (char*)realloc(buffer, (sw_size)capacity);
        }
        memcpy(buffer + length, chunk, (uint64_t)got);
        length += got;
    }
    sw_string* result = sw_string_from_literal(buffer, length);
    free(buffer);
    return result;
}

// 通用启动：envp/cwd 可为 NULL；err_fd_out 非空时 stderr 走单独管道。
static sw_string* sw_run_impl_env(
    sw_string* cmd, sw_array* args, char** envp, sw_string* dir, int* err_fd_out
) {
    int out_pipe[2];
    int err_pipe[2] = {-1, -1};
    if (pipe(out_pipe) != 0) {
        return sw_string_from_literal("", 0);
    }
    if (err_fd_out != NULL && pipe(err_pipe) != 0) {
        close(out_pipe[0]);
        close(out_pipe[1]);
        return sw_string_from_literal("", 0);
    }
    char** argv = sw_build_argv(cmd, args);
    int pid = fork();
    if (pid < 0) {
        close(out_pipe[0]);
        close(out_pipe[1]);
        if (err_fd_out != NULL) {
            close(err_pipe[0]);
            close(err_pipe[1]);
        }
        return sw_string_from_literal("", 0);
    }
    if (pid == 0) {
        if (dir != NULL) {
            extern int chdir(const char* path);
            chdir(dir->data);
        }
        dup2(out_pipe[1], 1);
        dup2(err_fd_out != NULL ? err_pipe[1] : out_pipe[1], 2);
        close(out_pipe[0]);
        close(out_pipe[1]);
        if (err_fd_out != NULL) {
            close(err_pipe[0]);
            close(err_pipe[1]);
        }
        if (envp != NULL) {
            extern int execve(const char* file, char* const argv[], char* const envp[]);
            // execve 不做 PATH 查找：先按 PATH 解析完整路径（与 Windows
            // CreateProcess 行为一致），找不到再原样尝试让 exec 报错。
            const char* resolved = argv[0];
            sw_string* found = os_which(sw_string_from_literal(argv[0], (int64_t)strlen(argv[0])));
            if (found != NULL && found->len > 0) {
                resolved = found->data;
            }
            execve(resolved, argv, envp);
        } else {
            execvp(argv[0], argv);
        }
        _exit(127);
    }
    close(out_pipe[1]);
    if (err_fd_out != NULL) {
        close(err_pipe[1]);
    }
    sw_string* result = sw_read_fd_all(out_pipe[0]);
    close(out_pipe[0]);
    if (err_fd_out != NULL) {
        *err_fd_out = err_pipe[0];
    }
    int status = 0;
    waitpid(pid, &status, 0);
    return result;
}

sw_string* sw_run_with_env(sw_string* cmd, sw_array* args, void* env_handle) {
    return sw_run_impl_env(cmd, args, sw_build_envp((sw_map*)env_handle), NULL, NULL);
}

sw_string* sw_run_in_dir(sw_string* cmd, sw_array* args, sw_string* dir) {
    return sw_run_impl_env(cmd, args, NULL, dir, NULL);
}

sw_array* sw_run_stdout_stderr(sw_string* cmd, sw_array* args) {
    int err_fd = -1;
    sw_string* out = sw_run_impl_env(cmd, args, NULL, NULL, &err_fd);
    sw_string* err = err_fd >= 0 ? sw_read_fd_all(err_fd) : sw_string_from_literal("", 0);
    if (err_fd >= 0) {
        close(err_fd);
    }
    sw_array* result = sw_array_new(8, 2);
    ((int64_t*)result->data)[0] = (int64_t)out;
    ((int64_t*)result->data)[1] = (int64_t)err;
    return result;
}

int64_t sw_is_process_running(int64_t pid) {
    extern int kill(int pid, int signal);
    return kill((int)pid, 0) == 0 ? 1 : 0;
}

#endif

// ---------------------------------------------------------------------------
// 进程交互（std/os）：启动子进程并保持 stdin/stdout 管道，逐步读写。
// 句柄为 0-63 的表索引（与文件 fd 表同模式）；行读取带内部缓冲。
// ---------------------------------------------------------------------------

#define SW_MAX_PROCS 64

typedef struct sw_proc {
#if defined(_WIN32)
    void* h_process;
    void* stdin_write;
    void* stdout_read;
#else
    int64_t pid;
    int64_t stdin_fd;
    int64_t stdout_fd;
#endif
    int64_t exited;
    int64_t eof;
    int64_t exit_code;
    char* line_buf;
    int64_t line_len;
    int64_t line_cap;
} sw_proc;

static sw_proc sw_procs[SW_MAX_PROCS];

static int64_t sw_proc_slot_alloc(void) {
    for (int64_t i = 0; i < SW_MAX_PROCS; i++) {
#if defined(_WIN32)
        if (sw_procs[i].h_process == NULL) {
            return i;
        }
#else
        if (sw_procs[i].pid == 0) {
            return i;
        }
#endif
    }
    return -1;
}

#if defined(_WIN32)

int64_t sw_process_open(sw_string* cmd, sw_array* args) {
    void* in_read = NULL;
    void* in_write = NULL;
    void* out_read = NULL;
    void* out_write = NULL;
    if (!CreatePipe(&in_read, &in_write, NULL, 0)) {
        return -1;
    }
    SetHandleInformation(in_read, 1, 1);  // 子进程读 stdin
    if (!CreatePipe(&out_read, &out_write, NULL, 0)) {
        CloseHandle(in_read);
        CloseHandle(in_write);
        return -1;
    }
    SetHandleInformation(out_write, 1, 1);  // 子进程写 stdout
    char* cmdline = sw_build_cmdline(cmd, args);
    sw_startup_info startup;
    memset(&startup, 0, sizeof(startup));
    startup.cb = sizeof(startup);
    startup.flags = 0x00000100u;  // STARTF_USESTDHANDLES
    startup.h_std_input = in_read;
    startup.h_std_output = out_write;
    startup.h_std_error = out_write;
    sw_proc_info info;
    memset(&info, 0, sizeof(info));
    int ok = CreateProcessA(NULL, cmdline, NULL, NULL, 1, 0, NULL, NULL, &startup, &info);
    CloseHandle(in_read);
    CloseHandle(out_write);
    if (!ok) {
        CloseHandle(in_write);
        CloseHandle(out_read);
        return -1;
    }
    CloseHandle(info.h_thread);
    int64_t slot = sw_proc_slot_alloc();
    if (slot < 0) {
        CloseHandle(info.h_process);
        CloseHandle(in_write);
        CloseHandle(out_read);
        return -1;
    }
    sw_proc* p = &sw_procs[slot];
    memset(p, 0, sizeof(*p));
    p->h_process = info.h_process;
    p->stdin_write = in_write;
    p->stdout_read = out_read;
    return slot;
}

#else  // POSIX（Linux / macOS）

int64_t sw_process_open(sw_string* cmd, sw_array* args) {
    int in_pipe[2] = {-1, -1};
    int out_pipe[2] = {-1, -1};
    if (pipe(in_pipe) != 0 || pipe(out_pipe) != 0) {
        if (in_pipe[0] >= 0) {
            close(in_pipe[0]);
            close(in_pipe[1]);
        }
        if (out_pipe[0] >= 0) {
            close(out_pipe[0]);
            close(out_pipe[1]);
        }
        return -1;
    }
    char** argv = sw_build_argv(cmd, args);
    int pid = fork();
    if (pid < 0) {
        close(in_pipe[0]);
        close(in_pipe[1]);
        close(out_pipe[0]);
        close(out_pipe[1]);
        return -1;
    }
    if (pid == 0) {
        dup2(in_pipe[0], 0);
        dup2(out_pipe[1], 1);
        dup2(out_pipe[1], 2);
        close(in_pipe[0]);
        close(in_pipe[1]);
        close(out_pipe[0]);
        close(out_pipe[1]);
        execvp(argv[0], argv);
        _exit(127);
    }
    close(in_pipe[0]);
    close(out_pipe[1]);
    int64_t slot = sw_proc_slot_alloc();
    if (slot < 0) {
        close(in_pipe[1]);
        close(out_pipe[0]);
        kill(pid, 9);
        waitpid(pid, NULL, 0);
        return -1;
    }
    sw_proc* p = &sw_procs[slot];
    memset(p, 0, sizeof(*p));
    p->pid = pid;
    p->stdin_fd = in_pipe[1];
    p->stdout_fd = out_pipe[0];
    return slot;
}

#endif

static int sw_proc_valid(int64_t proc) {
    if (proc < 0 || proc >= SW_MAX_PROCS) {
        return 0;
    }
    sw_proc* p = &sw_procs[proc];
#if defined(_WIN32)
    return p->h_process != NULL;
#else
    return p->pid != 0;
#endif
}

int64_t sw_process_write(int64_t proc, sw_string* text) {
    if (!sw_proc_valid(proc) || text == NULL) {
        return -1;
    }
    sw_proc* p = &sw_procs[proc];
#if defined(_WIN32)
    if (p->stdin_write == NULL) {
        return -1;
    }
    unsigned int written = 0;
    if (!WriteFile(p->stdin_write, text->data, (unsigned int)text->len, &written, NULL)) {
        return -1;
    }
    return (int64_t)written;
#else
    if (p->stdin_fd < 0) {
        return -1;
    }
    long n = write((int)p->stdin_fd, text->data, (unsigned long)text->len);
    return n < 0 ? -1 : (int64_t)n;
#endif
}

// 关闭子进程 stdin 写端（子进程读到 EOF；sort 等需要）。
int64_t sw_process_close_input(int64_t proc) {
    if (!sw_proc_valid(proc)) {
        return -1;
    }
    sw_proc* p = &sw_procs[proc];
#if defined(_WIN32)
    if (p->stdin_write == NULL) {
        return -1;
    }
    CloseHandle(p->stdin_write);
    p->stdin_write = NULL;
    return 0;
#else
    if (p->stdin_fd < 0) {
        return -1;
    }
    close((int)p->stdin_fd);
    p->stdin_fd = -1;
    return 0;
#endif
}

// 从子进程 stdout 读一块追加到行缓冲；返回字节数（0 EOF，-1 错误）。
static int64_t sw_proc_fill(sw_proc* p) {
    char chunk[4096];
#if defined(_WIN32)
    unsigned int got = 0;
    if (!ReadFile(p->stdout_read, chunk, sizeof(chunk), &got, NULL)) {
        return -1;
    }
    if (got == 0) {
        return 0;
    }
#else
    long got = read((int)p->stdout_fd, chunk, sizeof(chunk));
    if (got < 0) {
        return -1;
    }
    if (got == 0) {
        return 0;
    }
#endif
    if (p->line_len + (int64_t)got > p->line_cap) {
        int64_t new_cap = (p->line_len + (int64_t)got) * 2 + 64;
        char* bigger = (char*)realloc(p->line_buf, (sw_size)new_cap);
        if (bigger == NULL) {
            return -1;
        }
        p->line_buf = bigger;
        p->line_cap = new_cap;
    }
    memcpy(p->line_buf + p->line_len, chunk, (uint64_t)got);
    p->line_len += (int64_t)got;
    return got;
}

// 阻塞读一行（去 \n/\r）；EOF 返回空串。
sw_string* sw_process_read_line(int64_t proc) {
    if (!sw_proc_valid(proc)) {
        return sw_string_from_literal("", 0);
    }
    sw_proc* p = &sw_procs[proc];
    while (1) {
        for (int64_t i = 0; i < p->line_len; i++) {
            if (p->line_buf[i] == '\n') {
                int64_t len = i;
                if (len > 0 && p->line_buf[len - 1] == '\r') {
                    len--;
                }
                sw_string* result = sw_string_from_literal(p->line_buf, len);
                memmove(p->line_buf, p->line_buf + i + 1, (uint64_t)(p->line_len - i - 1));
                p->line_len -= (i + 1);
                return result;
            }
        }
        if (p->eof) {
            if (p->line_len > 0) {
                sw_string* result = sw_string_from_literal(p->line_buf, p->line_len);
                p->line_len = 0;
                return result;
            }
            return sw_string_from_literal("", 0);
        }
        int64_t got = sw_proc_fill(p);
        if (got == 0) {
            p->eof = 1;
            if (p->line_len == 0) {
                return sw_string_from_literal("", 0);
            }
        } else if (got < 0) {
            return sw_string_from_literal("", 0);
        }
    }
}

// 非阻塞读可用数据（缓冲优先；无缓冲时读一次）。
sw_string* sw_process_read_some(int64_t proc, int64_t max_bytes) {
    if (!sw_proc_valid(proc)) {
        return sw_string_from_literal("", 0);
    }
    sw_proc* p = &sw_procs[proc];
    if (max_bytes <= 0) {
        return sw_string_from_literal("", 0);
    }
    if (p->line_len > 0) {
        int64_t take = p->line_len < max_bytes ? p->line_len : max_bytes;
        sw_string* result = sw_string_from_literal(p->line_buf, take);
        memmove(p->line_buf, p->line_buf + take, (uint64_t)(p->line_len - take));
        p->line_len -= take;
        return result;
    }
    int64_t got = sw_proc_fill(p);
    if (got <= 0) {
        if (got == 0) {
            p->eof = 1;
        }
        return sw_string_from_literal("", 0);
    }
    int64_t take = got < max_bytes ? got : max_bytes;
    sw_string* result = sw_string_from_literal(p->line_buf, take);
    memmove(p->line_buf, p->line_buf + take, (uint64_t)(p->line_len - take));
    p->line_len -= take;
    return result;
}

// 非阻塞检查子进程 stdout 是否有数据可读：1 有 / 0 无 / -1 已 EOF 或无效。
int64_t sw_process_poll(int64_t proc) {
    if (!sw_proc_valid(proc)) {
        return -1;
    }
    sw_proc* p = &sw_procs[proc];
    if (p->eof) {
        return -1;
    }
    if (p->line_len > 0) {
        return 1;
    }
#if defined(_WIN32)
    // 匿名管道句柄在数据可读/对端关闭时 signaled：WaitForSingleObject(0) 探测。
    unsigned long result = WaitForSingleObject(p->stdout_read, 0);
    if (result == 0x00000102u) {  // WAIT_TIMEOUT：无数据
        return 0;
    }
    return 1;  // 有数据或 EOF（由后续 read 区分）
#else
    // select 读集合，0 超时
    unsigned char fds[128];
    memset(fds, 0, sizeof(fds));
    int wordsize = 64;
#if defined(__APPLE__)
    wordsize = 32;
#endif
    int fd = (int)p->stdout_fd;
    int word = fd / wordsize;
    int bit = fd % wordsize;
    if (wordsize == 64) {
        if (word >= 16) {
            return -1;
        }
        ((unsigned long long*)fds)[word] |= (1ULL << bit);
    } else {
        if (word >= 32) {
            return -1;
        }
        ((unsigned int*)fds)[word] |= (1u << bit);
    }
    struct {
        long tv_sec;
        long tv_usec;
    } tv;
    tv.tv_sec = 0;
    tv.tv_usec = 0;
    extern int select(int nfds, void* readfds, void* writefds, void* exceptfds, void* timeout);
    int result = select(fd + 1, fds, NULL, NULL, &tv);
    if (result < 0) {
        return -1;
    }
    if (result > 0) {
        return 1;
    }
    // 无数据：查进程是否已结束（读到 EOF 由下次 read 发现，这里先返回 0）
    return 0;
#endif
}

int64_t sw_process_wait(int64_t proc) {
    if (!sw_proc_valid(proc)) {
        return -1;
    }
    sw_proc* p = &sw_procs[proc];
    int64_t result = 0;
#if defined(_WIN32)
    WaitForSingleObject(p->h_process, 0xFFFFFFFFu);
    unsigned int code = 0;
    GetExitCodeProcess(p->h_process, &code);
    CloseHandle(p->h_process);
    if (p->stdin_write != NULL) {
        CloseHandle(p->stdin_write);
    }
    CloseHandle(p->stdout_read);
    result = (int64_t)code;
#else
    int status = 0;
    if (waitpid((int)p->pid, &status, 0) < 0) {
        return -1;
    }
    int code = status & 0x7f;
    result = code == 0 ? ((status >> 8) & 0xff) : (128 + code);
    if (p->stdin_fd >= 0) {
        close((int)p->stdin_fd);
    }
    close((int)p->stdout_fd);
#endif
    if (p->line_buf != NULL) {
        free(p->line_buf);
    }
    memset(p, 0, sizeof(*p));
    return result;
}

int64_t sw_process_kill(int64_t proc) {
    if (!sw_proc_valid(proc)) {
        return -1;
    }
    sw_proc* p = &sw_procs[proc];
#if defined(_WIN32)
    return TerminateProcess(p->h_process, 1) ? 0 : -1;
#else
    return kill((int)p->pid, 9) == 0 ? 0 : -1;
#endif
}

// 关闭句柄并释放槽（不等待子进程；进程继续运行则成为孤儿）。
int64_t sw_process_close(int64_t proc) {
    if (!sw_proc_valid(proc)) {
        return -1;
    }
    sw_proc* p = &sw_procs[proc];
#if defined(_WIN32)
    CloseHandle(p->h_process);
    if (p->stdin_write != NULL) {
        CloseHandle(p->stdin_write);
    }
    CloseHandle(p->stdout_read);
#else
    if (p->stdin_fd >= 0) {
        close((int)p->stdin_fd);
    }
    close((int)p->stdout_fd);
    waitpid((int)p->pid, NULL, 1);  // WNOHANG：能收就收
#endif
    if (p->line_buf != NULL) {
        free(p->line_buf);
    }
    memset(p, 0, sizeof(*p));
    return 0;
}

// ---------------------------------------------------------------------------
// 终端读键（std/console read_key）：完整按键（方向/功能键扩展码）。
// 扩展键码：上 1000 下 1001 左 1002 右 1003 Home 1004 End 1005
//           PgUp 1006 PgDn 1007 F1..F12 1011..1022；普通字符返回 0-255。
// ---------------------------------------------------------------------------

int64_t sw_read_key(void) {
#if defined(_WIN32)
    extern int _getch(void);
    int first = _getch();
    if (first == 0x00 || first == 0xE0) {
        int second = _getch();
        switch (second) {
            case 72: return 1000;  // Up
            case 80: return 1001;  // Down
            case 75: return 1002;  // Left
            case 77: return 1003;  // Right
            case 71: return 1004;  // Home
            case 79: return 1005;  // End
            case 73: return 1006;  // PgUp
            case 81: return 1007;  // PgDn
            case 82: return 1011;  // F1
            case 83: return 1012;  // F2
            case 84: return 1013;  // F3
            case 85: return 1014;  // F4
            case 86: return 1015;  // F5
            case 87: return 1016;  // F6
            case 88: return 1017;  // F7
            case 89: return 1018;  // F8
            case 90: return 1019;  // F9
            case 91: return 1020;  // F10
            case 92: return 1021;  // F11
            case 93: return 1022;  // F12
            default: return 2000 + second;
        }
    }
    return first;
#else
    extern long read(int fd, void* buffer, unsigned long count);
    unsigned char byte = 0;
    if (read(0, &byte, 1) != 1) {
        return -1;
    }
    if (byte != 0x1B) {
        return (int64_t)byte;
    }
    // ESC 开头：探测 CSI 序列（select 短超时等第二字节）
    unsigned char fds[128];
    memset(fds, 0, sizeof(fds));
    int wordsize = 64;
#if defined(__APPLE__)
    wordsize = 32;
#endif
    int word = 0 / wordsize;
    int bit = 0 % wordsize;
    if (wordsize == 64) {
        ((unsigned long long*)fds)[word] |= (1ULL << bit);
    } else {
        ((unsigned int*)fds)[word] |= (1u << bit);
    }
    struct {
        long tv_sec;
        long tv_usec;
    } tv;
    tv.tv_sec = 0;
    tv.tv_usec = 30000;  // 30ms
    extern int select(int nfds, void* readfds, void* writefds, void* exceptfds, void* timeout);
    if (select(1, fds, NULL, NULL, &tv) <= 0) {
        return 0x1B;  // 裸 ESC
    }
    unsigned char next = 0;
    if (read(0, &next, 1) != 1) {
        return 0x1B;
    }
    if (next != '[') {
        return 0x1B;  // 简单忽略非 CSI
    }
    unsigned char final_char = 0;
    if (read(0, &final_char, 1) != 1) {
        return 0x1B;
    }
    switch (final_char) {
        case 'A': return 1000;  // Up
        case 'B': return 1001;  // Down
        case 'C': return 1003;  // Right
        case 'D': return 1002;  // Left
        case 'H': return 1004;  // Home
        case 'F': return 1005;  // End
        default: return 0x1B;
    }
#endif
}

// ---------------------------------------------------------------------------
// 网络：TCP 阻塞式（Windows WinSock2 / POSIX socket）
// ---------------------------------------------------------------------------

typedef struct sw_sockaddr_in {
#if defined(__APPLE__)
    // macOS：sin_len(1 字节) + sin_family(1 字节)，端口在偏移 2。
    unsigned char sin_len;
    unsigned char family;
#else
    unsigned short family;
#endif
    unsigned short port;
    unsigned int addr;
    unsigned char zero[8];
} sw_sockaddr_in;

// addrinfo 布局按平台：
// - Windows：addrlen(size_t)@16、canonname@24、addr@32、next@40
// - macOS：addrlen(socklen_t=uint32)@16+pad、canonname@24、addr@32、next@40
// - Linux：addrlen(uint32)@16+pad、addr@24、canonname@32、next@40
typedef struct sw_addrinfo {
    int flags;
    int family;
    int socktype;
    int protocol;
#if defined(_WIN32)
    uint64_t addrlen;
    char* canonname;
    void* addr;
#elif defined(__APPLE__)
    unsigned int addrlen;
    unsigned int pad;
    char* canonname;
    void* addr;
#else
    uint64_t addrlen;
    void* addr;
    char* canonname;
#endif
    void* next;
} sw_addrinfo;

#if defined(_WIN32)
static int sw_net_started = 0;

static void sw_net_init(void) {
    if (!sw_net_started) {
        extern int WSAStartup(unsigned short version, void* data);
        unsigned char data[408];
        memset(data, 0, sizeof(data));
        if (WSAStartup(0x0202, data) == 0) {
            sw_net_started = 1;
        }
    }
}
#endif

static int sw_net_be16(int64_t value) {
    int v = (int)(value & 0xFFFF);
    return ((v & 0xFF) << 8) | ((v >> 8) & 0xFF);
}

int64_t sw_net_connect(sw_string* host, int64_t port) {
    if (host == NULL || port < 0 || port > 65535) {
        return -1;
    }
    char* host_copy = (char*)sw_gc_alloc((uint64_t)host->len + 1);
    memcpy(host_copy, host->data, (uint64_t)host->len);
    host_copy[host->len] = 0;
    char port_text[16];
    int plen = snprintf(port_text, sizeof(port_text), "%lld", (long long)port);
    (void)plen;
#if defined(_WIN32)
    extern int getaddrinfo(const char* node, const char* service, const void* hints, void** result);
    extern void freeaddrinfo(void* result);
    extern uintptr_t socket(int domain, int type, int protocol);
    extern int connect(uintptr_t s, const void* name, int namelen);
    extern int closesocket(uintptr_t s);
    sw_net_init();
#else
    extern int getaddrinfo(const char* node, const char* service, const void* hints, void** result);
    extern void freeaddrinfo(void* result);
    extern int socket(int domain, int type, int protocol);
    extern int connect(int s, const void* name, unsigned int namelen);
    extern int close(int s);
#endif
    sw_addrinfo hints;
    memset(&hints, 0, sizeof(hints));
    hints.family = 2;    // AF_INET
    hints.socktype = 1;  // SOCK_STREAM
    void* results = NULL;
    if (getaddrinfo(host_copy, port_text, &hints, &results) != 0 || results == NULL) {
        return -1;
    }
    sw_addrinfo* info = (sw_addrinfo*)results;
#if defined(_WIN32)
    uintptr_t sock = socket(info->family, info->socktype, info->protocol);
    if (sock == (uintptr_t)~0) {
        freeaddrinfo(results);
        return -1;
    }
    int ok = connect(sock, info->addr, (int)info->addrlen);
#else
    int sock = socket(info->family, info->socktype, info->protocol);
    if (sock < 0) {
        freeaddrinfo(results);
        return -1;
    }
    int ok = connect(sock, info->addr, (unsigned int)info->addrlen);
#endif
    freeaddrinfo(results);
    if (ok != 0) {
#if defined(_WIN32)
        closesocket(sock);
#else
        close(sock);
#endif
        return -1;
    }
    return (int64_t)sock;
}

int64_t sw_net_send(int64_t fd, sw_string* data) {
    if (fd < 0 || data == NULL) {
        return -1;
    }
#if defined(_WIN32)
    extern int send(uintptr_t s, const char* buf, int len, int flags);
    int64_t max = data->len > 0x7FFFFFFF ? 0x7FFFFFFF : data->len;
    int sent = send((uintptr_t)fd, data->data, (int)max, 0);
    return sent < 0 ? -1 : (int64_t)sent;
#else
    extern long send(int s, const void* buf, uintptr_t len, int flags);
    long sent = send((int)fd, data->data, (uintptr_t)data->len, 0);
    return sent < 0 ? -1 : (int64_t)sent;
#endif
}

sw_string* sw_net_recv(int64_t fd, int64_t max_bytes) {
    if (fd < 0 || max_bytes <= 0) {
        return sw_string_from_literal("", 0);
    }
    if (max_bytes > 16777216) {
        max_bytes = 16777216;
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)max_bytes + 1);
#if defined(_WIN32)
    extern int recv(uintptr_t s, char* buf, int len, int flags);
    int got = recv((uintptr_t)fd, buffer, (int)max_bytes, 0);
#else
    extern long recv(int s, void* buf, uintptr_t len, int flags);
    long got = recv((int)fd, buffer, (uintptr_t)max_bytes, 0);
#endif
    if (got <= 0) {
        return sw_string_from_literal("", 0);
    }
    buffer[got] = 0;
    return sw_string_from_literal(buffer, (int64_t)got);
}

int64_t sw_net_close(int64_t fd) {
#if defined(_WIN32)
    extern int closesocket(uintptr_t s);
    return closesocket((uintptr_t)fd) == 0 ? 0 : -1;
#else
    extern int close(int s);
    return close((int)fd) == 0 ? 0 : -1;
#endif
}

int64_t sw_net_listen(int64_t port) {
    if (port < 0 || port > 65535) {
        return -1;
    }
#if defined(_WIN32)
    extern uintptr_t socket(int domain, int type, int protocol);
    extern int bind(uintptr_t s, const void* name, int namelen);
    extern int listen(uintptr_t s, int backlog);
    extern int closesocket(uintptr_t s);
    sw_net_init();
    uintptr_t sock = socket(2, 1, 6);
    if (sock == (uintptr_t)~0) {
        return -1;
    }
    sw_sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
#if defined(__APPLE__)
    addr.sin_len = (unsigned char)sizeof(addr);
#endif
    addr.family = 2;
    addr.port = (unsigned short)sw_net_be16(port);
    addr.addr = 0;  // INADDR_ANY
    if (bind(sock, &addr, (int)sizeof(addr)) != 0 || listen(sock, 16) != 0) {
        closesocket(sock);
        return -1;
    }
    return (int64_t)sock;
#else
    extern int socket(int domain, int type, int protocol);
    extern int bind(int s, const void* name, unsigned int namelen);
    extern int listen(int s, int backlog);
    extern int close(int s);
    int sock = socket(2, 1, 6);
    if (sock < 0) {
        return -1;
    }
    sw_sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
#if defined(__APPLE__)
    addr.sin_len = (unsigned char)sizeof(addr);
#endif
    addr.family = 2;
    addr.port = (unsigned short)sw_net_be16(port);
    addr.addr = 0;
    if (bind(sock, &addr, (unsigned int)sizeof(addr)) != 0 || listen(sock, 16) != 0) {
        close(sock);
        return -1;
    }
    return sock;
#endif
}

int64_t sw_net_accept(int64_t fd) {
    unsigned char name[128];
    unsigned int namelen = sizeof(name);
#if defined(_WIN32)
    extern uintptr_t accept(uintptr_t s, void* name, int* namelen);
    uintptr_t client = accept((uintptr_t)fd, name, (int*)&namelen);
    return client == (uintptr_t)~0 ? -1 : (int64_t)client;
#else
    extern int accept(int s, void* name, unsigned int* namelen);
    int client = accept((int)fd, name, &namelen);
    return client < 0 ? -1 : client;
#endif
}

int64_t sw_net_port(int64_t fd) {
    unsigned char name[128];
    unsigned int namelen = sizeof(name);
#if defined(_WIN32)
    extern int getsockname(uintptr_t s, void* name, int* namelen);
    if (getsockname((uintptr_t)fd, name, (int*)&namelen) != 0) {
        return -1;
    }
#else
    extern int getsockname(int s, void* name, unsigned int* namelen);
    if (getsockname((int)fd, name, &namelen) != 0) {
        return -1;
    }
#endif
    unsigned short net_port = *(unsigned short*)(name + 2);
    return ((net_port & 0xFF) << 8) | (net_port >> 8);
}

// ---------------------------------------------------------------------------
// 网络增强（std/net）：连接超时、收发超时、可读字节、域名解析、对端信息、
// TCP keepalive、读到关闭、完整发送。
// ---------------------------------------------------------------------------

// 单 fd select 等待可写（connect 完成检测）。返回：1 可写 / 0 超时 / -1 错误。
static int sw_net_wait_writable(int64_t fd, int64_t timeout_ms) {
#if defined(_WIN32)
    typedef struct {
        unsigned int fd_count;
        uintptr_t fd_array[64];
    } sw_fdset;
    sw_fdset write_fds;
    memset(&write_fds, 0, sizeof(write_fds));
    write_fds.fd_count = 1;
    write_fds.fd_array[0] = (uintptr_t)fd;
    struct {
        int tv_sec;
        int tv_usec;
    } tv;
    tv.tv_sec = (int)(timeout_ms / 1000);
    tv.tv_usec = (int)((timeout_ms % 1000) * 1000);
    extern int select(int nfds, void* readfds, void* writefds, void* exceptfds, void* timeout);
    int result = select(0, NULL, &write_fds, NULL, timeout_ms > 0 ? &tv : NULL);
    return result > 0 ? 1 : (result == 0 ? 0 : -1);
#else
    unsigned char fds[128];  // fd_set：FD_SETSIZE=1024 位
    memset(fds, 0, sizeof(fds));
    int wordsize = 64;
#if defined(__APPLE__)
    wordsize = 32;
#endif
    int word = (int)(fd / wordsize);
    int bit = (int)(fd % wordsize);
    if (wordsize == 64) {
        if (word >= 16) {
            return -1;
        }
        ((unsigned long long*)fds)[word] |= (1ULL << bit);
    } else {
        if (word >= 32) {
            return -1;
        }
        ((unsigned int*)fds)[word] |= (1u << bit);
    }
    struct {
        long tv_sec;
        long tv_usec;
    } tv;
    tv.tv_sec = timeout_ms / 1000;
    tv.tv_usec = (timeout_ms % 1000) * 1000;
    extern int select(int nfds, void* readfds, void* writefds, void* exceptfds, void* timeout);
    int result = select((int)fd + 1, NULL, fds, NULL, timeout_ms > 0 ? &tv : NULL);
    return result > 0 ? 1 : (result == 0 ? 0 : -1);
#endif
}

static int sw_net_set_nonblocking(int64_t fd, int on) {
#if defined(_WIN32)
    extern int ioctlsocket(uintptr_t s, long cmd, unsigned long* argp);
    unsigned long mode = on ? 1 : 0;
    return ioctlsocket((uintptr_t)fd, 0x8004667Eu /*FIONBIO*/, &mode) == 0 ? 0 : -1;
#else
    extern int fcntl(int fd, int cmd, ...);
    int nonblock_flag = 0x800;  // O_NONBLOCK（Linux）
#if defined(__APPLE__)
    nonblock_flag = 0x4;        // O_NONBLOCK（macOS/BSD）
#endif
    int flags = fcntl((int)fd, 3 /*F_GETFL*/, 0);
    if (flags < 0) {
        return -1;
    }
    int updated = on ? (flags | nonblock_flag) : (flags & ~nonblock_flag);
    return fcntl((int)fd, 4 /*F_SETFL*/, updated) == 0 ? 0 : -1;
#endif
}

// 检查 connect 结果（非阻塞 connect 完成后读 SO_ERROR）。
// SOL_SOCKET: Linux=1, macOS/BSD/Windows=0xFFFF
#if defined(__APPLE__)
#define SW_SOL_SOCKET 0xFFFF
#else
#define SW_SOL_SOCKET 1
#endif
static int sw_net_connect_result(int64_t fd) {
#if defined(_WIN32)
    extern int getsockopt(uintptr_t s, int level, int optname, char* optval, int* optlen);
    int error = 0;
    int len = sizeof(error);
    if (getsockopt((uintptr_t)fd, 0xFFFF /*SOL_SOCKET*/, 0x1007 /*SO_ERROR*/, (char*)&error, &len) != 0) {
        return -1;
    }
    return error == 0 ? 0 : -1;
#else
    extern int getsockopt(int s, int level, int optname, void* optval, unsigned int* optlen);
    int error = 0;
    unsigned int len = sizeof(error);
    int so_error = 4;  // SO_ERROR（Linux）
#if defined(__APPLE__)
    so_error = 0x1007;  // SO_ERROR（macOS/BSD）
#endif
    if (getsockopt((int)fd, SW_SOL_SOCKET, so_error, &error, &len) != 0) {
        return -1;
    }
    return error == 0 ? 0 : -1;
#endif
}

// 带超时的 TCP 连接（timeout_ms<=0 不限时）。返回 fd；失败/超时返回 -1。
static char sw_net_last_error_buf[256] = "";
static void sw_net_record_error(const char* msg, int64_t detail) {
    snprintf(sw_net_last_error_buf, sizeof(sw_net_last_error_buf), "%s (%lld)", msg, (long long)detail);
}
static int64_t sw_net_errno_value(void) {
#if defined(_WIN32)
    extern int WSAGetLastError(void);
    return WSAGetLastError();
#elif defined(__APPLE__)
    extern int* __error(void);
    return *__error();
#else
    extern int* __errno_location(void);
    return *__errno_location();
#endif
}
sw_string* sw_net_last_error(void) {
    return sw_string_from_literal(sw_net_last_error_buf, (int64_t)strlen(sw_net_last_error_buf));
}
int64_t sw_net_connect_timeout(sw_string* host, int64_t port, int64_t timeout_ms) {
    if (host == NULL || port < 0 || port > 65535) {
        sw_net_record_error("参数非法", port);
        return -1;
    }
    char* host_copy = (char*)sw_gc_alloc((uint64_t)host->len + 1);
    memcpy(host_copy, host->data, (uint64_t)host->len);
    host_copy[host->len] = 0;
    char port_text[16];
    snprintf(port_text, sizeof(port_text), "%lld", (long long)port);
#if defined(_WIN32)
    extern int getaddrinfo(const char* node, const char* service, const void* hints, void** result);
    extern void freeaddrinfo(void* result);
    extern uintptr_t socket(int domain, int type, int protocol);
    extern int connect(uintptr_t s, const void* name, int namelen);
    extern int closesocket(uintptr_t s);
    extern int WSAGetLastError(void);
    sw_net_init();
#else
    extern int getaddrinfo(const char* node, const char* service, const void* hints, void** result);
    extern void freeaddrinfo(void* result);
    extern int socket(int domain, int type, int protocol);
    extern int connect(int s, const void* name, unsigned int namelen);
    extern int close(int s);
#endif
    sw_addrinfo hints;
    memset(&hints, 0, sizeof(hints));
    hints.family = 2;
    hints.socktype = 1;
    void* results = NULL;
    int gai_rc = getaddrinfo(host_copy, port_text, &hints, &results);
    if (gai_rc != 0 || results == NULL) {
        sw_net_record_error("getaddrinfo", gai_rc);
        return -1;
    }
    sw_addrinfo* info = (sw_addrinfo*)results;
#if defined(_WIN32)
    uintptr_t sock = socket(info->family, info->socktype, info->protocol);
    if (sock == (uintptr_t)~0) {
        sw_net_record_error("socket", sw_net_errno_value());
        freeaddrinfo(results);
        return -1;
    }
#else
    int sock = socket(info->family, info->socktype, info->protocol);
    if (sock < 0) {
        sw_net_record_error("socket", sw_net_errno_value());
        freeaddrinfo(results);
        return -1;
    }
#endif
    sw_net_set_nonblocking((int64_t)sock, 1);
    int ok;
#if defined(_WIN32)
    ok = connect(sock, info->addr, (int)info->addrlen);
#else
    ok = connect(sock, info->addr, (unsigned int)info->addrlen);
#endif
    if (ok != 0) {
        int in_progress = 0;
#if defined(_WIN32)
        in_progress = WSAGetLastError() == 10035;  // WSAEWOULDBLOCK
#elif defined(__APPLE__)
        extern int* __error(void);
        in_progress = *__error() == 36;  // EINPROGRESS（macOS）
#else
        extern int* __errno_location(void);
        in_progress = *__errno_location() == 115;  // EINPROGRESS（Linux）
#endif
        if (!in_progress) {
            sw_net_record_error("connect", sw_net_errno_value());
            freeaddrinfo(results);
#if defined(_WIN32)
            closesocket(sock);
#else
            close(sock);
#endif
            return -1;
        }
        int ready = sw_net_wait_writable((int64_t)sock, timeout_ms);
        int conn_error = ready > 0 ? sw_net_connect_result((int64_t)sock) : 0;
        if (ready <= 0 || conn_error != 0) {
            sw_net_record_error(ready <= 0 ? "select" : "so_error", ready <= 0 ? ready : conn_error);
            freeaddrinfo(results);
#if defined(_WIN32)
            closesocket(sock);
#else
            close(sock);
#endif
            return -1;
        }
    }
    freeaddrinfo(results);
    sw_net_set_nonblocking((int64_t)sock, 0);
    return (int64_t)sock;
}

// 收发超时（SO_RCVTIMEO/SO_SNDTIMEO，毫秒；0 表示不限时）。
int64_t sw_net_set_recv_timeout(int64_t fd, int64_t timeout_ms) {
#if defined(_WIN32)
    extern int setsockopt(uintptr_t s, int level, int optname, const char* optval, int optlen);
    unsigned int ms = (unsigned int)(timeout_ms < 0 ? 0 : timeout_ms);
    return setsockopt((uintptr_t)fd, 0xFFFF /*SOL_SOCKET*/, 0x1006 /*SO_RCVTIMEO*/, (const char*)&ms, sizeof(ms)) == 0 ? 0 : -1;
#else
    extern int setsockopt(int s, int level, int optname, const void* optval, unsigned int optlen);
    struct {
        long tv_sec;
        long tv_usec;
    } tv;
    tv.tv_sec = timeout_ms / 1000;
    tv.tv_usec = (timeout_ms % 1000) * 1000;
    int optname = 20;  // SO_RCVTIMEO（Linux）
#if defined(__APPLE__)
    optname = 0x1006;  // SO_RCVTIMEO（macOS/BSD）
#endif
    return setsockopt((int)fd, SW_SOL_SOCKET, optname, &tv, sizeof(tv)) == 0 ? 0 : -1;
#endif
}

int64_t sw_net_set_send_timeout(int64_t fd, int64_t timeout_ms) {
#if defined(_WIN32)
    extern int setsockopt(uintptr_t s, int level, int optname, const char* optval, int optlen);
    unsigned int ms = (unsigned int)(timeout_ms < 0 ? 0 : timeout_ms);
    return setsockopt((uintptr_t)fd, 0xFFFF /*SOL_SOCKET*/, 0x1005 /*SO_SNDTIMEO*/, (const char*)&ms, sizeof(ms)) == 0 ? 0 : -1;
#else
    extern int setsockopt(int s, int level, int optname, const void* optval, unsigned int optlen);
    struct {
        long tv_sec;
        long tv_usec;
    } tv;
    tv.tv_sec = timeout_ms / 1000;
    tv.tv_usec = (timeout_ms % 1000) * 1000;
    int optname = 21;  // SO_SNDTIMEO（Linux，19 是 SO_SNDLOWAT）
#if defined(__APPLE__)
    optname = 0x1005;  // SO_SNDTIMEO（macOS/BSD）
#endif
    return setsockopt((int)fd, SW_SOL_SOCKET, optname, &tv, sizeof(tv)) == 0 ? 0 : -1;
#endif
}

// 可读字节数（FIONREAD）。
int64_t sw_net_available(int64_t fd) {
#if defined(_WIN32)
    extern int ioctlsocket(uintptr_t s, long cmd, unsigned long* argp);
    unsigned long n = 0;
    if (ioctlsocket((uintptr_t)fd, 0x4004667Fu /*FIONREAD*/, &n) != 0) {
        sw_net_record_error("net_available", sw_net_errno_value());
        return -1;
    }
    return (int64_t)n;
#elif defined(__APPLE__)
    // macOS: ioctl(FIONREAD) 在此环境 EFAULT，改用 getsockopt(SO_NREAD)。
    extern int getsockopt(int s, int level, int optname, void* optval, unsigned int* optlen);
    int n = 0;
    unsigned int len = sizeof(n);
    if (getsockopt((int)fd, SW_SOL_SOCKET, 0x1020 /*SO_NREAD*/, &n, &len) != 0) {
        sw_net_record_error("net_available", sw_net_errno_value());
        return -1;
    }
    return (int64_t)n;
#else
    extern int ioctl(int fd, unsigned long request, void* arg);
    int n = 0;
    unsigned long request = 0x541B;  // FIONREAD (Linux)
    if (ioctl((int)fd, request, &n) != 0) {
        sw_net_record_error("net_available", sw_net_errno_value());
        return -1;
    }
    return (int64_t)n;
#endif
}

// 域名解析为 IPv4 点分字符串；失败返回空串。
sw_string* sw_net_resolve(sw_string* host) {
    if (host == NULL) {
        return sw_string_from_literal("", 0);
    }
    char* host_copy = (char*)sw_gc_alloc((uint64_t)host->len + 1);
    memcpy(host_copy, host->data, (uint64_t)host->len);
    host_copy[host->len] = 0;
#if defined(_WIN32)
    extern int getaddrinfo(const char* node, const char* service, const void* hints, void** result);
    extern void freeaddrinfo(void* result);
    sw_net_init();
#else
    extern int getaddrinfo(const char* node, const char* service, const void* hints, void** result);
    extern void freeaddrinfo(void* result);
#endif
    sw_addrinfo hints;
    memset(&hints, 0, sizeof(hints));
    hints.family = 2;
    hints.socktype = 1;
    void* results = NULL;
    if (getaddrinfo(host_copy, NULL, &hints, &results) != 0 || results == NULL) {
        return sw_string_from_literal("", 0);
    }
    sw_addrinfo* info = (sw_addrinfo*)results;
    unsigned int ip = *(unsigned int*)((char*)info->addr + 4);  // sockaddr_in 的 sin_addr
    freeaddrinfo(results);
    char text[32];
    int used = snprintf(
        text, sizeof(text), "%u.%u.%u.%u",
        ip & 0xFFu, (ip >> 8) & 0xFFu, (ip >> 16) & 0xFFu, (ip >> 24) & 0xFFu
    );
    return sw_string_from_literal(text, used);
}

// 对端 IP（getpeername，IPv4 点分）；失败返回空串。
sw_string* sw_net_peer_ip(int64_t fd) {
    unsigned char name[128];
    unsigned int namelen = sizeof(name);
#if defined(_WIN32)
    extern int getpeername(uintptr_t s, void* name, int* namelen);
    if (getpeername((uintptr_t)fd, name, (int*)&namelen) != 0) {
        return sw_string_from_literal("", 0);
    }
#else
    extern int getpeername(int s, void* name, unsigned int* namelen);
    if (getpeername((int)fd, name, &namelen) != 0) {
        return sw_string_from_literal("", 0);
    }
#endif
    unsigned int ip = *(unsigned int*)(name + 4);
    char text[32];
    int used = snprintf(
        text, sizeof(text), "%u.%u.%u.%u",
        ip & 0xFFu, (ip >> 8) & 0xFFu, (ip >> 16) & 0xFFu, (ip >> 24) & 0xFFu
    );
    return sw_string_from_literal(text, used);
}

// 对端端口（getpeername）；失败返回 -1。
int64_t sw_net_peer_port(int64_t fd) {
    unsigned char name[128];
    unsigned int namelen = sizeof(name);
#if defined(_WIN32)
    extern int getpeername(uintptr_t s, void* name, int* namelen);
    if (getpeername((uintptr_t)fd, name, (int*)&namelen) != 0) {
        return -1;
    }
#else
    extern int getpeername(int s, void* name, unsigned int* namelen);
    if (getpeername((int)fd, name, &namelen) != 0) {
        return -1;
    }
#endif
    unsigned short net_port = *(unsigned short*)(name + 2);
    return ((net_port & 0xFF) << 8) | (net_port >> 8);
}

// TCP keepalive 选项（SO_KEEPALIVE）。
int64_t sw_net_set_keepalive(int64_t fd, int64_t enabled) {
    int value = enabled ? 1 : 0;
#if defined(_WIN32)
    extern int setsockopt(uintptr_t s, int level, int optname, const char* optval, int optlen);
    return setsockopt((uintptr_t)fd, 0xFFFF /*SOL_SOCKET*/, 8 /*SO_KEEPALIVE*/, (const char*)&value, sizeof(value)) == 0 ? 0 : -1;
#else
    extern int setsockopt(int s, int level, int optname, const void* optval, unsigned int optlen);
    int optname = 9;  // SO_KEEPALIVE（Linux）
#if defined(__APPLE__)
    optname = 0x0008;  // SO_KEEPALIVE（macOS/BSD）
#endif
    return setsockopt((int)fd, SW_SOL_SOCKET, optname, &value, sizeof(value)) == 0 ? 0 : -1;
#endif
}

// 读取直到对端关闭（HTTP 响应体等）；失败/EOF 返回已读内容。
sw_string* sw_net_recv_until_close(int64_t fd) {
    char chunk[4096];
    char* buffer = (char*)malloc(4096);
    int64_t capacity = 4096;
    int64_t length = 0;
    while (1) {
#if defined(_WIN32)
        extern int recv(uintptr_t s, char* buf, int len, int flags);
        int got = recv((uintptr_t)fd, chunk, sizeof(chunk), 0);
#else
        extern long recv(int s, void* buf, uintptr_t len, int flags);
        long got = recv((int)fd, chunk, sizeof(chunk), 0);
#endif
        if (got <= 0) {
            break;
        }
        if (length + (int64_t)got > capacity) {
            capacity = (length + (int64_t)got) * 2;
            buffer = (char*)realloc(buffer, (sw_size)capacity);
        }
        memcpy(buffer + length, chunk, (uint64_t)got);
        length += (int64_t)got;
    }
    sw_string* result = sw_string_from_literal(buffer, length);
    free(buffer);
    return result;
}

// 完整发送（循环直到全部写入）；返回发送字节数；失败 -1。
int64_t sw_net_send_all(int64_t fd, sw_string* data) {
    if (fd < 0 || data == NULL) {
        return -1;
    }
    int64_t total = 0;
    while (total < data->len) {
#if defined(_WIN32)
        extern int send(uintptr_t s, const char* buf, int len, int flags);
        int64_t max = (data->len - total) > 0x7FFFFFFF ? 0x7FFFFFFF : (data->len - total);
        int sent = send((uintptr_t)fd, data->data + total, (int)max, 0);
#else
        extern long send(int s, const void* buf, uintptr_t len, int flags);
        long sent = send((int)fd, data->data + total, (uintptr_t)(data->len - total), 0);
#endif
        if (sent <= 0) {
            return -1;
        }
        total += (int64_t)sent;
    }
    return total;
}

// ---------------------------------------------------------------------------
// 标准库扩充：字符串工具（remove_prefix/remove_suffix/大小写判定/capitalize）
// ---------------------------------------------------------------------------

sw_string* remove_prefix(sw_string* text, sw_string* prefix) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    if (prefix == NULL || prefix->len == 0 || text->len < prefix->len) {
        return sw_string_from_literal(text->data, text->len);
    }
    if (memcmp(text->data, prefix->data, (uint64_t)prefix->len) == 0) {
        return sw_string_from_literal(text->data + prefix->len, text->len - prefix->len);
    }
    return sw_string_from_literal(text->data, text->len);
}

sw_string* remove_suffix(sw_string* text, sw_string* suffix) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    if (suffix == NULL || suffix->len == 0 || text->len < suffix->len) {
        return sw_string_from_literal(text->data, text->len);
    }
    int64_t offset = text->len - suffix->len;
    if (memcmp(text->data + offset, suffix->data, (uint64_t)suffix->len) == 0) {
        return sw_string_from_literal(text->data, offset);
    }
    return sw_string_from_literal(text->data, text->len);
}

int64_t is_upper(sw_string* text) {
    if (text == NULL || text->len == 0) {
        return 0;
    }
    int has_upper = 0;
    for (int64_t i = 0; i < text->len; i++) {
        unsigned char c = (unsigned char)text->data[i];
        if (c >= 'a' && c <= 'z') {
            return 0;
        }
        if (c >= 'A' && c <= 'Z') {
            has_upper = 1;
        }
    }
    return has_upper ? 1 : 0;
}

int64_t is_lower(sw_string* text) {
    if (text == NULL || text->len == 0) {
        return 0;
    }
    int has_lower = 0;
    for (int64_t i = 0; i < text->len; i++) {
        unsigned char c = (unsigned char)text->data[i];
        if (c >= 'A' && c <= 'Z') {
            return 0;
        }
        if (c >= 'a' && c <= 'z') {
            has_lower = 1;
        }
    }
    return has_lower ? 1 : 0;
}

int64_t is_digit(sw_string* text) {
    if (text == NULL || text->len == 0) {
        return 0;
    }
    for (int64_t i = 0; i < text->len; i++) {
        unsigned char c = (unsigned char)text->data[i];
        if (c < '0' || c > '9') {
            return 0;
        }
    }
    return 1;
}

sw_string* capitalize(sw_string* text) {
    if (text == NULL || text->len == 0) {
        return sw_string_from_literal("", 0);
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len + 1);
    memcpy(buffer, text->data, (uint64_t)text->len);
    buffer[text->len] = 0;
    if (buffer[0] >= 'a' && buffer[0] <= 'z') {
        buffer[0] = buffer[0] - 'a' + 'A';
    }
    return sw_string_from_literal(buffer, text->len);
}

// ---------------------------------------------------------------------------
// 标准库扩充：数组 contains / index_of
// ---------------------------------------------------------------------------

int64_t contains_int(sw_array* items, int64_t value) {
    if (items == NULL) {
        return 0;
    }
    int64_t* data = (int64_t*)items->data;
    for (int64_t i = 0; i < items->len; i++) {
        if (data[i] == value) {
            return 1;
        }
    }
    return 0;
}

int64_t contains_float(sw_array* items, double value) {
    if (items == NULL) {
        return 0;
    }
    double* data = (double*)items->data;
    for (int64_t i = 0; i < items->len; i++) {
        if (data[i] == value) {
            return 1;
        }
    }
    return 0;
}

int64_t contains_string(sw_array* items, sw_string* value) {
    if (items == NULL || value == NULL) {
        return 0;
    }
    sw_string** data = (sw_string**)items->data;
    for (int64_t i = 0; i < items->len; i++) {
        if (string_eq(data[i], value)) {
            return 1;
        }
    }
    return 0;
}

int64_t index_of_int(sw_array* items, int64_t value) {
    if (items == NULL) {
        return -1;
    }
    int64_t* data = (int64_t*)items->data;
    for (int64_t i = 0; i < items->len; i++) {
        if (data[i] == value) {
            return i;
        }
    }
    return -1;
}

int64_t index_of_float(sw_array* items, double value) {
    if (items == NULL) {
        return -1;
    }
    double* data = (double*)items->data;
    for (int64_t i = 0; i < items->len; i++) {
        if (data[i] == value) {
            return i;
        }
    }
    return -1;
}

int64_t index_of_string(sw_array* items, sw_string* value) {
    if (items == NULL || value == NULL) {
        return -1;
    }
    sw_string** data = (sw_string**)items->data;
    for (int64_t i = 0; i < items->len; i++) {
        if (string_eq(data[i], value)) {
            return i;
        }
    }
    return -1;
}

// ---------------------------------------------------------------------------
// 文本处理库（火山文本处理类参考）：UTF-8 字节安全，中文可用
// ---------------------------------------------------------------------------

static int sw_is_whitespace_byte(unsigned char c) {
    return c == ' ' || c == '\t' || c == '\r' || c == '\n';
}

// 全角空格 U+3000 的 UTF-8 编码是 EF 80 80。
static int sw_is_fullwidth_space(const char* text, int64_t offset, int64_t len) {
    return offset + 2 < len && (unsigned char)text[offset] == 0xEF
        && (unsigned char)text[offset + 1] == 0x80
        && (unsigned char)text[offset + 2] == 0x80;
}

int64_t is_blank(sw_string* text) {
    if (text == NULL) {
        return 1;
    }
    for (int64_t i = 0; i < text->len; i++) {
        unsigned char c = (unsigned char)text->data[i];
        if (sw_is_whitespace_byte(c)) {
            continue;
        }
        if (sw_is_fullwidth_space(text->data, i, text->len)) {
            i += 2;
            continue;
        }
        return 0;
    }
    return 1;
}

sw_string* strip_whitespace(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len + 1);
    int64_t out = 0;
    for (int64_t i = 0; i < text->len; i++) {
        unsigned char c = (unsigned char)text->data[i];
        if (sw_is_whitespace_byte(c)) {
            continue;
        }
        if (sw_is_fullwidth_space(text->data, i, text->len)) {
            i += 2;
            continue;
        }
        buffer[out++] = text->data[i];
    }
    buffer[out] = 0;
    return sw_string_from_literal(buffer, out);
}

// 首个 start 标记后、其后首个 end 标记前的内容（字节位置，找不到返回空）。
sw_string* substring_between(sw_string* text, sw_string* start, sw_string* end) {
    if (text == NULL || start == NULL || end == NULL || start->len == 0) {
        return sw_string_from_literal("", 0);
    }
    int64_t begin = index_of(text, start);
    if (begin < 0) {
        return sw_string_from_literal("", 0);
    }
    begin += start->len;
    sw_string tail = { text->data + begin, text->len - begin };
    int64_t finish = index_of(&tail, end);
    if (finish < 0) {
        return sw_string_from_literal("", 0);
    }
    return sw_string_from_literal(text->data + begin, finish);
}

// 从右往左：最后一个 end 标记前的、其前最后一个 start 标记后的内容。
sw_string* substring_between_last(sw_string* text, sw_string* start, sw_string* end) {
    if (text == NULL || start == NULL || end == NULL || start->len == 0) {
        return sw_string_from_literal("", 0);
    }
    // 从右往左：先找最后一个 start（右侧标记），再在其左侧找最后一个 end。
    int64_t begin = -1;
    for (int64_t i = 0; i + start->len <= text->len; i++) {
        int64_t ok = 1;
        for (int64_t j = 0; j < start->len; j++) {
            if (text->data[i + j] != start->data[j]) {
                ok = 0;
                break;
            }
        }
        if (ok) {
            begin = i;
        }
    }
    if (begin < 0) {
        return sw_string_from_literal("", 0);
    }
    int64_t finish = -1;
    for (int64_t i = 0; i + end->len <= begin; i++) {
        int64_t ok = 1;
        for (int64_t j = 0; j < end->len; j++) {
            if (text->data[i + j] != end->data[j]) {
                ok = 0;
                break;
            }
        }
        if (ok) {
            finish = i;
        }
    }
    if (finish < 0) {
        return sw_string_from_literal("", 0);
    }
    finish += end->len;
    if (finish > begin) {
        return sw_string_from_literal("", 0);
    }
    return sw_string_from_literal(text->data + finish, begin - finish);
}

// 批量提取 start 与 end 之间的全部内容，返回 string[]。
sw_array* extract_between(sw_string* text, sw_string* start, sw_string* end) {
    sw_array* result = sw_array_new(8, 0);
    if (text == NULL || start == NULL || end == NULL || start->len == 0) {
        return result;
    }
    int64_t position = 0;
    while (position + start->len <= text->len) {
        sw_string tail = { text->data + position, text->len - position };
        int64_t begin = index_of(&tail, start);
        if (begin < 0) {
            break;
        }
        int64_t content = begin + start->len;
        sw_string after = { text->data + position + content, text->len - position - content };
        int64_t finish = index_of(&after, end);
        if (finish < 0) {
            break;
        }
        if (finish > 0) {
            sw_string* piece = sw_string_from_literal(
                text->data + position + content,
                finish
            );
            sw_array* bigger = sw_array_new(8, result->len + 1);
            memcpy(bigger->data, result->data, (sw_size)((uint64_t)result->len * 8));
            bigger->len = result->len;
            result = bigger;
            ((int64_t*)result->data)[result->len++] = (int64_t)piece;
        }
        position += content + finish + end->len;
    }
    return result;
}

sw_string* before(sw_string* text, sw_string* marker) {
    if (text == NULL || marker == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t position = index_of(text, marker);
    if (position < 0) {
        return sw_string_from_literal("", 0);
    }
    return sw_string_from_literal(text->data, position);
}

sw_string* after(sw_string* text, sw_string* marker) {
    if (text == NULL || marker == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t position = index_of(text, marker);
    if (position < 0) {
        return sw_string_from_literal("", 0);
    }
    int64_t offset = position + marker->len;
    return sw_string_from_literal(text->data + offset, text->len - offset);
}

sw_string* before_last(sw_string* text, sw_string* marker) {
    if (text == NULL || marker == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t position = -1;
    for (int64_t i = 0; i + marker->len <= text->len; i++) {
        int64_t ok = 1;
        for (int64_t j = 0; j < marker->len; j++) {
            if (text->data[i + j] != marker->data[j]) {
                ok = 0;
                break;
            }
        }
        if (ok) {
            position = i;
        }
    }
    if (position < 0) {
        return sw_string_from_literal("", 0);
    }
    return sw_string_from_literal(text->data, position);
}

sw_string* after_last(sw_string* text, sw_string* marker) {
    if (text == NULL || marker == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t position = -1;
    for (int64_t i = 0; i + marker->len <= text->len; i++) {
        int64_t ok = 1;
        for (int64_t j = 0; j < marker->len; j++) {
            if (text->data[i + j] != marker->data[j]) {
                ok = 0;
                break;
            }
        }
        if (ok) {
            position = i;
        }
    }
    if (position < 0) {
        return sw_string_from_literal("", 0);
    }
    int64_t offset = position + marker->len;
    return sw_string_from_literal(text->data + offset, text->len - offset);
}

// 第 index 个字符（UTF-8 码点）的代码值；越界返回 -1。
int64_t char_code(sw_string* text, int64_t index) {
    if (text == NULL || index < 0) {
        return -1;
    }
    int64_t offset = 0;
    for (int64_t i = 0; i < index; i++) {
        if (offset >= text->len) {
            return -1;
        }
        offset += sw_utf8_char_length(text->data, offset, text->len);
    }
    if (offset >= text->len) {
        return -1;
    }
    unsigned char c = (unsigned char)text->data[offset];
    if (c < 0x80) {
        return c;
    }
    int64_t length = sw_utf8_char_length(text->data, offset, text->len);
    if (length == 2) {
        return ((c & 0x1F) << 6) | ((unsigned char)text->data[offset + 1] & 0x3F);
    }
    if (length == 3) {
        return ((c & 0x0F) << 12)
            | (((unsigned char)text->data[offset + 1] & 0x3F) << 6)
            | ((unsigned char)text->data[offset + 2] & 0x3F);
    }
    if (length == 4) {
        return ((c & 0x07) << 18)
            | (((unsigned char)text->data[offset + 1] & 0x3F) << 12)
            | (((unsigned char)text->data[offset + 2] & 0x3F) << 6)
            | ((unsigned char)text->data[offset + 3] & 0x3F);
    }
    return -1;
}

// 连续子文本替换：pairs 为 varargs 打包数组，元素成对（tag, value），
// 依次替换（前一轮结果再进入下一轮）。
sw_string* replace_pairs(sw_string* text, sw_array* pairs) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    sw_string* result = sw_string_from_literal(text->data, text->len);
    if (pairs == NULL) {
        return result;
    }
    int64_t* data = (int64_t*)pairs->data;
    int64_t count = pairs->len / 2;
    for (int64_t i = 0; i < count; i++) {
        // varargs 打包：每元素占两槽（tag, value），成对取 value。
        int64_t pair = i * 2;
        sw_string* from = (sw_string*)data[pair * 2 + 1];
        sw_string* to = (sw_string*)data[(pair + 1) * 2 + 1];
        if (from != NULL && to != NULL) {
            result = replace(result, from, to);
        }
    }
    return result;
}

// ---------------------------------------------------------------------------
// 时间处理库（火山时间类参考）：本地时区字段/格式化/间隔
// ---------------------------------------------------------------------------

// field：0=年 1=月 2=星期 3=日 4=时 5=分 6=秒。
// Windows 用 FileTimeToSystemTime（SYSTEMTIME 偏移：year@0 mon@2 wday@4
// day@6 hour@8 min@10 sec@12）；POSIX 用 localtime_r（tm 偏移：
// sec@0 min@4 hour@8 day@12 mon@16 year@20 wday@24）。
static int sw_time_field(int64_t seconds, int field) {
#if defined(_WIN32)
    unsigned char st[16];
    sw_unix_to_local_systemtime(st, seconds);
    static const int offsets[] = {0, 2, 4, 6, 8, 10, 12};
    return *(unsigned short*)(st + offsets[field]);
#else
    extern void* localtime_r(const void* time, void* tm);
    unsigned char tm[64];
    unsigned char t[8];
    *(int64_t*)t = seconds;
    if (localtime_r(t, tm) == NULL) {
        return -1;
    }
    static const int offsets[] = {20, 16, 24, 12, 8, 4, 0};
    int value = *(int*)(tm + offsets[field]);
    if (field == 0) {
        return value + 1900;  // tm_year 是从 1900 起的年数
    }
    if (field == 1) {
        return value + 1;     // tm_mon 是 0-11
    }
    return value;
#endif
}

// 本地日历时间增减（DST 安全）：在本地时间字段上加偏移后重新构造时间戳。
static int64_t sw_shift_local_time(
    int64_t seconds,
    int64_t days,
    int64_t hours,
    int64_t minutes,
    int64_t secs
) {
#if defined(_WIN32)
    unsigned char st[16];
    sw_unix_to_local_systemtime(st, seconds);
    *(unsigned short*)(st + 6) = (unsigned short)(*(unsigned short*)(st + 6) + (int)days);
    *(unsigned short*)(st + 8) = (unsigned short)(*(unsigned short*)(st + 8) + (int)hours);
    *(unsigned short*)(st + 10) = (unsigned short)(*(unsigned short*)(st + 10) + (int)minutes);
    *(unsigned short*)(st + 12) = (unsigned short)(*(unsigned short*)(st + 12) + (int)secs);
    extern int TzSpecificLocalTimeToSystemTime(const void* tz, const void* local, void* utc);
    extern int SystemTimeToFileTime(const void* st_, void* ft);
    unsigned char utc_st[16];
    if (!TzSpecificLocalTimeToSystemTime(NULL, st, utc_st)) {
        return -1;
    }
    unsigned char ft[8];
    if (!SystemTimeToFileTime(utc_st, ft)) {
        return -1;
    }
    uint64_t since_1601 =
        ((uint64_t)(*(unsigned int*)(ft + 4)) << 32) | (*(unsigned int*)ft);
    return (int64_t)(since_1601 / 10000000 - 11644473600ULL);
#else
    extern long mktime(void* tm);
    unsigned char tm[64];
    unsigned char t[8];
    *(int64_t*)t = seconds;
    extern void* localtime_r(const void* time, void* tm_out);
    if (localtime_r(t, tm) == NULL) {
        return -1;
    }
    *(int*)(tm + 0) += (int)secs;
    *(int*)(tm + 4) += (int)minutes;
    *(int*)(tm + 8) += (int)hours;
    *(int*)(tm + 12) += (int)days;
    *(int*)(tm + 32) = -1;
    long result = mktime(tm);
    return result == (long)-1 ? -1 : (int64_t)result;
#endif
}

int64_t year_of(int64_t seconds) {
    int value = sw_time_field(seconds, 0);
    return value < 0 ? -1 : value;
}

int64_t month_of(int64_t seconds) {
    int value = sw_time_field(seconds, 1);
    return value < 0 ? -1 : value;
}

int64_t day_of(int64_t seconds) {
    return sw_time_field(seconds, 3);
}

int64_t hour_of(int64_t seconds) {
    return sw_time_field(seconds, 4);
}

int64_t minute_of(int64_t seconds) {
    return sw_time_field(seconds, 5);
}

int64_t second_of(int64_t seconds) {
    return sw_time_field(seconds, 6);
}

// 星期几：0=周日 … 6=周六。
int64_t weekday_of(int64_t seconds) {
    return sw_time_field(seconds, 2);
}

// 中文星期：日/一/二/三/四/五/六。
sw_string* weekday_cn(int64_t seconds) {
    static const char* names[] = {"日", "一", "二", "三", "四", "五", "六"};
    int wday = sw_time_field(seconds, 2);
    if (wday < 0 || wday > 6) {
        return sw_string_from_literal("", 0);
    }
    return sw_string_from_literal(names[wday], 3);
}

sw_string* time_string(int64_t seconds) {
    char buffer[16];
    int h = sw_time_field(seconds, 4);
    int m = sw_time_field(seconds, 5);
    int s = sw_time_field(seconds, 6);
    if (h < 0) {
        return sw_string_from_literal("", 0);
    }
    snprintf(buffer, sizeof(buffer), "%02d:%02d:%02d", h, m, s);
    return sw_string_from_literal(buffer, 8);
}

// ISO 风格本地时间："YYYY-MM-DDTHH:MM:SS"。
sw_string* iso_string(int64_t seconds) {
    char buffer[32];
    int year = sw_time_field(seconds, 0);
    int month = sw_time_field(seconds, 1);
    int day = sw_time_field(seconds, 3);
    int hour = sw_time_field(seconds, 4);
    int minute = sw_time_field(seconds, 5);
    int second = sw_time_field(seconds, 6);
    if (year < 0) {
        return sw_string_from_literal("", 0);
    }
    snprintf(
        buffer,
        sizeof(buffer),
        "%04d-%02d-%02dT%02d:%02d:%02d",
        year,
        month,
        day,
        hour,
        minute,
        second
    );
    return sw_string_from_literal(buffer, 19);
}

// 秒数转时长文本：90 → "00:01:30"；text_mode 用 "00时01分30秒"；
// include_days 前缀 "N天 "。
sw_string* format_duration(int64_t seconds, int64_t include_days, int64_t text_mode) {
    if (seconds < 0) {
        seconds = 0;
    }
    int64_t days = seconds / 86400;
    int64_t hour = (seconds % 86400) / 3600;
    int64_t minute = (seconds % 3600) / 60;
    int64_t second = seconds % 60;
    char buffer[64];
    if (include_days) {
        if (text_mode) {
            snprintf(
                buffer,
                sizeof(buffer),
                "%lld天 %02lld时%02lld分%02lld秒",
                (long long)days,
                (long long)hour,
                (long long)minute,
                (long long)second
            );
        } else {
            snprintf(
                buffer,
                sizeof(buffer),
                "%lld天 %02lld:%02lld:%02lld",
                (long long)days,
                (long long)hour,
                (long long)minute,
                (long long)second
            );
        }
    } else if (text_mode) {
        snprintf(
            buffer,
            sizeof(buffer),
            "%02lld时%02lld分%02lld秒",
            (long long)hour,
            (long long)minute,
            (long long)second
        );
    } else {
        snprintf(
            buffer,
            sizeof(buffer),
            "%02lld:%02lld:%02lld",
            (long long)hour,
            (long long)minute,
            (long long)second
        );
    }
    return sw_string_from_literal(buffer, (int64_t)strlen(buffer));
}

int64_t days_in_month(int64_t year, int64_t month) {
    static const int days[] = {31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
    if (month < 1 || month > 12) {
        return -1;
    }
    if (month == 2 && ((year % 4 == 0 && year % 100 != 0) || year % 400 == 0)) {
        return 29;
    }
    return days[month - 1];
}

int64_t days_in_year(int64_t year) {
    if ((year % 4 == 0 && year % 100 != 0) || year % 400 == 0) {
        return 366;
    }
    return 365;
}

int64_t shift_time(
    int64_t seconds,
    int64_t days,
    int64_t hours,
    int64_t minutes,
    int64_t secs
) {
    return sw_shift_local_time(seconds, days, hours, minutes, secs);
}

// 两个时间戳的间隔，按单位换算（0秒 1分 2时 3天），结果 = sec2 - sec1。
int64_t time_diff(int64_t sec1, int64_t sec2, int64_t unit) {
    int64_t diff = sec2 - sec1;
    switch (unit) {
        case 1:
            return diff / 60;
        case 2:
            return diff / 3600;
        case 3:
            return diff / 86400;
        default:
            return diff;
    }
}

// 进程启动以来毫秒数（高精度单调时钟）。
int64_t uptime_ms(void) {
#if defined(_WIN32)
    extern int QueryPerformanceFrequency(int64_t* freq);
    extern int QueryPerformanceCounter(int64_t* counter);
    int64_t freq = 0;
    int64_t counter = 0;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&counter);
    if (freq <= 0) {
        return 0;
    }
    return (int64_t)((counter * 1000) / freq);
#else
    extern int clock_gettime(int clock_id, void* ts);
    unsigned char ts[16];
#if defined(__APPLE__)
    const int clock_monotonic = 6;  // macOS CLOCK_MONOTONIC
#else
    const int clock_monotonic = 1;  // Linux CLOCK_MONOTONIC
#endif
    if (clock_gettime(clock_monotonic, ts) != 0) {
        return 0;
    }
    int64_t sec = *(int64_t*)ts;
    int64_t nsec = *(int64_t*)(ts + 8);
    return sec * 1000 + nsec / 1000000;
#endif
}

// ---------------------------------------------------------------------------
// 命令行参数解析（flag 风格）
// args 为 string[]（元素为 sw_string*），支持：
//   flag_has(args, "--verbose") / flag_has(args, "-v")
//   flag_value(args, "--port")  // "--port=8080" 或 "--port 8080"
// ---------------------------------------------------------------------------

static int64_t sw_arg_string_eq(sw_string* arg, const char* text) {
    if (arg == NULL) {
        return 0;
    }
    int64_t len = (int64_t)strlen(text);
    if (arg->len != len) {
        return 0;
    }
    for (int64_t i = 0; i < len; i++) {
        if (arg->data[i] != text[i]) {
            return 0;
        }
    }
    return 1;
}

static int64_t sw_arg_starts_with(sw_string* arg, const char* prefix) {
    if (arg == NULL) {
        return 0;
    }
    int64_t len = (int64_t)strlen(prefix);
    if (arg->len < len) {
        return 0;
    }
    for (int64_t i = 0; i < len; i++) {
        if (arg->data[i] != prefix[i]) {
            return 0;
        }
    }
    return 1;
}

// 是否存在该 flag：等于 name 或形如 name=value。
int64_t flag_has(sw_array* args, sw_string* name) {
    if (args == NULL || name == NULL) {
        return 0;
    }
    // name 转 C 字符串（仅 ASCII 比较；flag 名一般为 ASCII）。
    char* name_c = (char*)sw_gc_alloc((uint64_t)name->len + 1);
    for (int64_t i = 0; i < name->len; i++) {
        name_c[i] = name->data[i];
    }
    name_c[name->len] = 0;
    int64_t* data = (int64_t*)args->data;
    for (int64_t i = 0; i < args->len; i++) {
        sw_string* arg = (sw_string*)data[i];
        if (sw_arg_string_eq(arg, name_c)) {
            return 1;
        }
        if (sw_arg_starts_with(arg, name_c)) {
            // 检查 name=value
            int64_t name_len = name->len;
            if (arg->len > name_len && arg->data[name_len] == '=') {
                return 1;
            }
        }
    }
    return 0;
}

// 取 flag 值：--name=value 或 --name value；无则返回 NULL。
sw_string* flag_value(sw_array* args, sw_string* name) {
    if (args == NULL || name == NULL) {
        return NULL;
    }
    char* name_c = (char*)sw_gc_alloc((uint64_t)name->len + 1);
    for (int64_t i = 0; i < name->len; i++) {
        name_c[i] = name->data[i];
    }
    name_c[name->len] = 0;
    int64_t* data = (int64_t*)args->data;
    for (int64_t i = 0; i < args->len; i++) {
        sw_string* arg = (sw_string*)data[i];
        if (sw_arg_starts_with(arg, name_c)) {
            int64_t name_len = name->len;
            if (arg->len > name_len && arg->data[name_len] == '=') {
                // name=value：切出 value。
                sw_string* value = sw_string_from_literal(
                    arg->data + name_len + 1,
                    arg->len - name_len - 1
                );
                return value;
            }
            if (sw_arg_string_eq(arg, name_c) && i + 1 < args->len) {
                // 独立 flag，取下一个参数为值。
                sw_string* next = (sw_string*)data[i + 1];
                if (next != NULL && next->len > 0 && next->data[0] != '-') {
                    return next;
                }
                return NULL;
            }
        }
    }
    return NULL;
}

// ---------------------------------------------------------------------------
// 正则表达式（最小引擎，纯自包含）
// 支持：字面量、. * + ? [] 字符类（范围/取反）、^ $ 锚点、() 捕获分组、
//       | 交替、\d \w \s \D \W \S 与转义字符。文本按 Unicode 码点处理，
//       中文安全。
// ---------------------------------------------------------------------------

typedef enum {
    RX_CHAR,
    RX_ANY,
    RX_CLASS,
    RX_CONCAT,
    RX_ALT,
    RX_STAR,
    RX_PLUS,
    RX_QUEST,
    RX_GROUP,
    RX_ANCHOR_BEGIN,
    RX_ANCHOR_END,
    RX_DIGIT,
    RX_WORD,
    RX_SPACE,
    RX_NDIGIT,
    RX_NWORD,
    RX_NSPACE
} rx_kind;

typedef struct rx_node {
    rx_kind kind;
    int64_t ch;                  // RX_CHAR
    struct rx_node** children;   // RX_CONCAT / RX_ALT
    int64_t child_count;
    struct rx_node* child;       // RX_STAR / RX_PLUS / RX_QUEST / RX_GROUP
    int64_t negate;              // RX_CLASS
    int64_t* class_chars;        // RX_CLASS 字符集（排序去重）
    int64_t class_count;
    int64_t group_index;         // RX_GROUP
} rx_node;

typedef struct {
    int64_t* data;
    int64_t len;
} rx_codepoints;

// UTF-8 解码：把字符串转成码点数组。
static rx_codepoints rx_decode(const char* data, int64_t len) {
    rx_codepoints cp;
    cp.data = (int64_t*)sw_gc_alloc((uint64_t)(len + 1) * 8);
    cp.len = 0;
    int64_t i = 0;
    while (i < len) {
        unsigned char c = (unsigned char)data[i];
        int64_t code = 0;
        int64_t extra = 0;
        if (c < 0x80) {
            code = c;
        } else if ((c & 0xE0) == 0xC0) {
            code = c & 0x1F;
            extra = 1;
        } else if ((c & 0xF0) == 0xE0) {
            code = c & 0x0F;
            extra = 2;
        } else if ((c & 0xF8) == 0xF0) {
            code = c & 0x07;
            extra = 3;
        } else {
            code = c;
        }
        for (int64_t k = 1; k <= extra && i + k < len; k++) {
            code = (code << 6) | ((unsigned char)data[i + k] & 0x3F);
        }
        cp.data[cp.len++] = code;
        i += extra + 1;
    }
    return cp;
}

static rx_node* rx_new(rx_kind kind) {
    rx_node* node = (rx_node*)sw_gc_alloc(sizeof(rx_node));
    node->kind = kind;
    node->ch = 0;
    node->children = NULL;
    node->child_count = 0;
    node->child = NULL;
    node->negate = 0;
    node->class_chars = NULL;
    node->class_count = 0;
    node->group_index = 0;
    return node;
}

static rx_node* rx_concat(rx_node** items, int64_t count) {
    if (count == 1) {
        return items[0];
    }
    rx_node* node = rx_new(RX_CONCAT);
    node->children = (rx_node**)sw_gc_alloc((uint64_t)count * sizeof(rx_node*));
    node->child_count = count;
    for (int64_t i = 0; i < count; i++) {
        node->children[i] = items[i];
    }
    return node;
}

static int64_t rx_class_lookup(int64_t* chars, int64_t count, int64_t ch) {
    for (int64_t i = 0; i < count; i++) {
        if (chars[i] == ch) {
            return 1;
        }
    }
    return 0;
}

// 解析器状态：模式码点、长度、位置、分组计数。
typedef struct {
    int64_t* data;
    int64_t len;
    int64_t pos;
    int64_t groups;
} rx_parser;

static rx_node* rx_parse_alt(rx_parser* p);
static rx_node* rx_parse_escape(rx_parser* p);

static void rx_class_add(rx_node* node, int64_t ch) {
    if (rx_class_lookup(node->class_chars, node->class_count, ch)) {
        return;
    }
    int64_t* bigger = (int64_t*)sw_gc_alloc((uint64_t)(node->class_count + 1) * 8);
    for (int64_t i = 0; i < node->class_count; i++) {
        bigger[i] = node->class_chars[i];
    }
    bigger[node->class_count] = ch;
    node->class_chars = bigger;
    node->class_count++;
}

static void rx_class_range(rx_node* node, int64_t lo, int64_t hi) {
    if (lo > hi) {
        int64_t t = lo;
        lo = hi;
        hi = t;
    }
    for (int64_t ch = lo; ch <= hi && ch < 0x110000; ch++) {
        rx_class_add(node, ch);
    }
}

// 解析字符类 [...]；进入时 pos 指向 '['，返回时 pos 指向 ']' 之后。
static rx_node* rx_parse_class(rx_parser* p) {
    rx_node* node = rx_new(RX_CLASS);
    p->pos++;  // 跳过 '['
    if (p->pos < p->len && p->data[p->pos] == '^') {
        node->negate = 1;
        p->pos++;
    }
    while (p->pos < p->len && p->data[p->pos] != ']') {
        int64_t ch = p->data[p->pos];
        if (ch == '\\' && p->pos + 1 < p->len) {
            p->pos++;
            int64_t esc = p->data[p->pos];
            if (esc == 'd') {
                rx_class_range(node, '0', '9');
            } else if (esc == 'w') {
                rx_class_range(node, 'a', 'z');
                rx_class_range(node, 'A', 'Z');
                rx_class_range(node, '0', '9');
                rx_class_add(node, '_');
            } else if (esc == 's') {
                rx_class_add(node, ' ');
                rx_class_add(node, '\t');
                rx_class_add(node, '\n');
                rx_class_add(node, '\r');
                rx_class_add(node, '\f');
                rx_class_add(node, '\v');
            } else {
                rx_class_add(node, esc);
            }
            p->pos++;
            continue;
        }
        // 范围：a-z
        if (p->pos + 2 < p->len && p->data[p->pos + 1] == '-' &&
            p->data[p->pos + 2] != ']') {
            int64_t hi = p->data[p->pos + 2];
            rx_class_range(node, ch, hi);
            p->pos += 3;
        } else {
            rx_class_add(node, ch);
            p->pos++;
        }
    }
    if (p->pos < p->len) {
        p->pos++;  // 跳过 ']'
    }
    return node;
}

static rx_node* rx_parse_atom(rx_parser* p) {
    if (p->pos >= p->len) {
        return NULL;
    }
    int64_t ch = p->data[p->pos];
    if (ch == '(') {
        p->pos++;
        rx_node* inner = rx_parse_alt(p);
        if (p->pos < p->len && p->data[p->pos] == ')') {
            p->pos++;
        }
        rx_node* group = rx_new(RX_GROUP);
        group->child = inner;
        group->group_index = p->groups++;
        return group;
    }
    if (ch == '[') {
        return rx_parse_class(p);
    }
    if (ch == '^') {
        p->pos++;
        return rx_new(RX_ANCHOR_BEGIN);
    }
    if (ch == '$') {
        p->pos++;
        return rx_new(RX_ANCHOR_END);
    }
    if (ch == '.') {
        p->pos++;
        return rx_new(RX_ANY);
    }
    if (ch == '\\') {
        return rx_parse_escape(p);
    }
    if (ch == '|' || ch == ')' || ch == '*' || ch == '+' || ch == '?') {
        return NULL;
    }
    rx_node* node = rx_new(RX_CHAR);
    node->ch = ch;
    p->pos++;
    return node;
}

static rx_node* rx_parse_escape(rx_parser* p) {
    p->pos++;  // 跳过 '\'
    if (p->pos >= p->len) {
        return NULL;
    }
    int64_t esc = p->data[p->pos];
    rx_node* node = NULL;
    switch (esc) {
        case 'd':
            node = rx_new(RX_DIGIT);
            break;
        case 'w':
            node = rx_new(RX_WORD);
            break;
        case 's':
            node = rx_new(RX_SPACE);
            break;
        case 'D':
            node = rx_new(RX_NDIGIT);
            break;
        case 'W':
            node = rx_new(RX_NWORD);
            break;
        case 'S':
            node = rx_new(RX_NSPACE);
            break;
        default:
            node = rx_new(RX_CHAR);
            node->ch = esc;
            break;
    }
    p->pos++;
    return node;
}

static rx_node* rx_parse_repeat(rx_parser* p) {
    rx_node* atom = rx_parse_atom(p);
    if (atom == NULL) {
        return NULL;
    }
    if (p->pos >= p->len) {
        return atom;
    }
    int64_t q = p->data[p->pos];
    if (q == '*') {
        rx_node* star = rx_new(RX_STAR);
        star->child = atom;
        p->pos++;
        return star;
    }
    if (q == '+') {
        rx_node* plus = rx_new(RX_PLUS);
        plus->child = atom;
        p->pos++;
        return plus;
    }
    if (q == '?') {
        rx_node* quest = rx_new(RX_QUEST);
        quest->child = atom;
        p->pos++;
        return quest;
    }
    return atom;
}

static rx_node* rx_parse_concat(rx_parser* p) {
    rx_node** items = (rx_node**)sw_gc_alloc(64 * sizeof(rx_node*));
    int64_t count = 0;
    while (p->pos < p->len) {
        int64_t ch = p->data[p->pos];
        if (ch == '|' || ch == ')') {
            break;
        }
        rx_node* item = rx_parse_repeat(p);
        if (item == NULL) {
            break;
        }
        if (count < 63) {
            items[count++] = item;
        }
    }
    return rx_concat(items, count);
}

static rx_node* rx_parse_alt(rx_parser* p) {
    rx_node* first = rx_parse_concat(p);
    if (p->pos >= p->len || p->data[p->pos] != '|') {
        return first;
    }
    rx_node** branches = (rx_node**)sw_gc_alloc(32 * sizeof(rx_node*));
    int64_t branch_count = 0;
    branches[branch_count++] = first;
    while (p->pos < p->len && p->data[p->pos] == '|') {
        p->pos++;
        rx_node* branch = rx_parse_concat(p);
        branches[branch_count++] = branch;
    }
    rx_node* alt = rx_new(RX_ALT);
    alt->children = branches;
    alt->child_count = branch_count;
    return alt;
}

// 编译模式字符串为 AST，并记录分组数。
static rx_node* rx_compile(const char* data, int64_t len, int64_t* groups) {
    rx_codepoints cp = rx_decode(data, len);
    rx_parser p;
    p.data = cp.data;
    p.len = cp.len;
    p.pos = 0;
    p.groups = 1;  // 0 号组为整个匹配
    rx_node* node = rx_parse_alt(&p);
    *groups = p.groups;
    return node;
}

typedef struct {
    int64_t* caps;
    int64_t count;
} rx_captures;

static int64_t rx_match_seq(
    rx_node** nodes,
    int64_t count,
    int64_t index,
    int64_t* text,
    int64_t len,
    int64_t pos,
    int64_t* caps,
    int64_t cap_count
);

// 匹配 node 从 pos 开始（码点索引），返回结束位置；失败返回 -1。
// 仅处理原子节点与 CONCAT/ALT/GROUP；量词由 rx_match_seq 展开。
static int64_t rx_match_node(
    rx_node* node,
    int64_t* text,
    int64_t len,
    int64_t pos,
    int64_t* caps,
    int64_t cap_count
) {
    if (node == NULL) {
        return pos;
    }
    switch (node->kind) {
        case RX_CHAR:
            if (pos < len && text[pos] == node->ch) {
                return pos + 1;
            }
            return -1;
        case RX_ANY:
            if (pos < len) {
                return pos + 1;
            }
            return -1;
        case RX_ANCHOR_BEGIN:
            return pos == 0 ? pos : -1;
        case RX_ANCHOR_END:
            return pos == len ? pos : -1;
        case RX_DIGIT:
            if (pos < len && text[pos] >= '0' && text[pos] <= '9') {
                return pos + 1;
            }
            return -1;
        case RX_NDIGIT:
            if (pos < len && !(text[pos] >= '0' && text[pos] <= '9')) {
                return pos + 1;
            }
            return -1;
        case RX_WORD:
            if (pos < len) {
                int64_t c = text[pos];
                if ((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
                    (c >= '0' && c <= '9') || c == '_') {
                    return pos + 1;
                }
            }
            return -1;
        case RX_NWORD:
            if (pos < len) {
                int64_t c = text[pos];
                if (!((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
                      (c >= '0' && c <= '9') || c == '_')) {
                    return pos + 1;
                }
            }
            return -1;
        case RX_SPACE:
            if (pos < len) {
                int64_t c = text[pos];
                if (c == ' ' || c == '\t' || c == '\n' || c == '\r' ||
                    c == '\f' || c == '\v') {
                    return pos + 1;
                }
            }
            return -1;
        case RX_NSPACE:
            if (pos < len) {
                int64_t c = text[pos];
                if (c != ' ' && c != '\t' && c != '\n' && c != '\r' &&
                    c != '\f' && c != '\v') {
                    return pos + 1;
                }
            }
            return -1;
        case RX_CLASS: {
            if (pos >= len) {
                return -1;
            }
            int64_t hit = rx_class_lookup(node->class_chars, node->class_count, text[pos]);
            if (node->negate ? !hit : hit) {
                return pos + 1;
            }
            return -1;
        }
        case RX_CONCAT: {
            return rx_match_seq(
                node->children,
                node->child_count,
                0,
                text,
                len,
                pos,
                caps,
                cap_count
            );
        }
        case RX_ALT: {
            for (int64_t i = 0; i < node->child_count; i++) {
                rx_node* wrapper = rx_new(RX_CONCAT);
                wrapper->children = (rx_node**)sw_gc_alloc(sizeof(rx_node*));
                wrapper->child_count = 1;
                wrapper->children[0] = node->children[i];
                int64_t p = rx_match_seq(
                    wrapper->children,
                    wrapper->child_count,
                    0,
                    text,
                    len,
                    pos,
                    caps,
                    cap_count
                );
                if (p >= 0) {
                    return p;
                }
            }
            return -1;
        }
        case RX_GROUP: {
            int64_t g = node->group_index;
            int64_t old_start = g * 2 < cap_count ? caps[g * 2] : -1;
            int64_t old_end = g * 2 + 1 < cap_count ? caps[g * 2 + 1] : -1;
            if (g * 2 < cap_count) {
                caps[g * 2] = pos;
            }
            rx_node* wrapper = rx_new(RX_CONCAT);
            wrapper->children = (rx_node**)sw_gc_alloc(sizeof(rx_node*));
            wrapper->child_count = 1;
            wrapper->children[0] = node->child;
            int64_t p = rx_match_seq(
                wrapper->children,
                wrapper->child_count,
                0,
                text,
                len,
                pos,
                caps,
                cap_count
            );
            if (p >= 0) {
                if (g * 2 + 1 < cap_count) {
                    caps[g * 2 + 1] = p;
                }
                return p;
            }
            if (g * 2 < cap_count) {
                caps[g * 2] = old_start;
            }
            if (g * 2 + 1 < cap_count) {
                caps[g * 2 + 1] = old_end;
            }
            return -1;
        }
        case RX_STAR:
        case RX_PLUS:
        case RX_QUEST:
            // 量词只在序列匹配（rx_match_seq）中展开。
            return -1;
        default:
            return -1;
    }
}

// 序列匹配：按顺序匹配 nodes[index..count)，量词贪心并回溯配合后续。
static int64_t rx_match_seq(
    rx_node** nodes,
    int64_t count,
    int64_t index,
    int64_t* text,
    int64_t len,
    int64_t pos,
    int64_t* caps,
    int64_t cap_count
) {
    if (index >= count) {
        return pos;
    }
    rx_node* node = nodes[index];
    switch (node->kind) {
        case RX_STAR: {
            int64_t* positions = (int64_t*)sw_gc_alloc((uint64_t)(len + 1) * 8);
            int64_t pos_count = 0;
            int64_t p = pos;
            positions[pos_count++] = p;
            while (pos_count < len + 1) {
                int64_t q = rx_match_node(node->child, text, len, p, caps, cap_count);
                if (q < 0 || q == p) {
                    break;
                }
                positions[pos_count++] = q;
                p = q;
            }
            for (int64_t i = pos_count - 1; i >= 0; i--) {
                int64_t r = rx_match_seq(
                    nodes,
                    count,
                    index + 1,
                    text,
                    len,
                    positions[i],
                    caps,
                    cap_count
                );
                if (r >= 0) {
                    return r;
                }
            }
            return -1;
        }
        case RX_PLUS: {
            int64_t q = rx_match_node(node->child, text, len, pos, caps, cap_count);
            if (q < 0) {
                return -1;
            }
            int64_t* positions = (int64_t*)sw_gc_alloc((uint64_t)(len + 1) * 8);
            int64_t pos_count = 0;
            int64_t p = q;
            positions[pos_count++] = q;
            while (pos_count < len + 1) {
                int64_t r = rx_match_node(node->child, text, len, p, caps, cap_count);
                if (r < 0 || r == p) {
                    break;
                }
                positions[pos_count++] = r;
                p = r;
            }
            for (int64_t i = pos_count - 1; i >= 0; i--) {
                int64_t r = rx_match_seq(
                    nodes,
                    count,
                    index + 1,
                    text,
                    len,
                    positions[i],
                    caps,
                    cap_count
                );
                if (r >= 0) {
                    return r;
                }
            }
            return -1;
        }
        case RX_QUEST: {
            int64_t q = rx_match_node(node->child, text, len, pos, caps, cap_count);
            if (q >= 0) {
                int64_t r = rx_match_seq(
                    nodes,
                    count,
                    index + 1,
                    text,
                    len,
                    q,
                    caps,
                    cap_count
                );
                if (r >= 0) {
                    return r;
                }
            }
            return rx_match_seq(nodes, count, index + 1, text, len, pos, caps, cap_count);
        }
        default: {
            int64_t p = rx_match_node(node, text, len, pos, caps, cap_count);
            if (p < 0) {
                return -1;
            }
            return rx_match_seq(
                nodes,
                count,
                index + 1,
                text,
                len,
                p,
                caps,
                cap_count
            );
        }
    }
}

// 匹配整个表达式（从 pos 起），返回结束位置或 -1。
static int64_t rx_match_full(
    rx_node* node,
    int64_t* text,
    int64_t len,
    int64_t pos,
    int64_t* caps,
    int64_t cap_count
) {
    rx_node* wrapper = rx_new(RX_CONCAT);
    wrapper->children = (rx_node**)sw_gc_alloc(sizeof(rx_node*));
    wrapper->child_count = 1;
    wrapper->children[0] = node;
    return rx_match_seq(
        wrapper->children,
        wrapper->child_count,
        0,
        text,
        len,
        pos,
        caps,
        cap_count
    );
}

// 在文本中查找第一个匹配，返回 [start, end)（码点索引）；无匹配返回 0。
// matched_start/matched_end 为码点索引。
static int64_t rx_search(
    rx_node* node,
    int64_t* text,
    int64_t len,
    int64_t* matched_start,
    int64_t* matched_end
) {
    for (int64_t start = 0; start <= len; start++) {
        int64_t caps[16];
        for (int64_t i = 0; i < 16; i++) {
            caps[i] = -1;
        }
        int64_t end = rx_match_full(node, text, len, start, caps, 16);
        if (end >= 0) {
            *matched_start = start;
            *matched_end = end;
            return 1;
        }
    }
    return 0;
}

// 从码点区间切出字符串。
static sw_string* rx_slice(int64_t* text, int64_t start, int64_t end) {
    // 需要把码点重新编码为 UTF-8。
    int64_t cap = (end - start) * 4 + 1;
    char* buffer = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t used = 0;
    for (int64_t i = start; i < end; i++) {
        int64_t ch = text[i];
        if (ch < 0x80) {
            buffer[used++] = (char)ch;
        } else if (ch < 0x800) {
            buffer[used++] = (char)(0xC0 | (ch >> 6));
            buffer[used++] = (char)(0x80 | (ch & 0x3F));
        } else if (ch < 0x10000) {
            buffer[used++] = (char)(0xE0 | (ch >> 12));
            buffer[used++] = (char)(0x80 | ((ch >> 6) & 0x3F));
            buffer[used++] = (char)(0x80 | (ch & 0x3F));
        } else {
            buffer[used++] = (char)(0xF0 | (ch >> 18));
            buffer[used++] = (char)(0x80 | ((ch >> 12) & 0x3F));
            buffer[used++] = (char)(0x80 | ((ch >> 6) & 0x3F));
            buffer[used++] = (char)(0x80 | (ch & 0x3F));
        }
    }
    buffer[used] = 0;
    return sw_string_from_literal(buffer, used);
}

// 正则匹配：整个文本是否完全匹配模式（bool）。
static int64_t rx_regex_match_impl(sw_string* text, sw_string* pattern);
static sw_string* rx_regex_find_impl(sw_string* text, sw_string* pattern);
static sw_array* rx_regex_find_all_impl(sw_string* text, sw_string* pattern);
static sw_string* rx_regex_replace_impl(sw_string* text, sw_string* pattern, sw_string* replacement);
static sw_array* rx_regex_split_impl(sw_string* text, sw_string* pattern);
static sw_array* rx_regex_captures_impl(sw_string* text, sw_string* pattern);

int64_t regex_match(sw_string* text, sw_string* pattern) {
    // GC 暂停：rx 编译树/码点数组为 C 临时对象，保守式扫描可能误回收。
    sw_gc_disable();
    int64_t result = rx_regex_match_impl(text, pattern);
    sw_gc_enable();
    return result;
}

static int64_t rx_regex_match_impl(sw_string* text, sw_string* pattern) {
    if (text == NULL || pattern == NULL) {
        return 0;
    }
    int64_t groups = 0;
    rx_node* node = rx_compile(pattern->data, pattern->len, &groups);
    rx_codepoints cp = rx_decode(text->data, text->len);
    int64_t caps[16];
    for (int64_t i = 0; i < 16; i++) {
        caps[i] = -1;
    }
    int64_t end = rx_match_full(node, cp.data, cp.len, 0, caps, 16);
    return end == cp.len ? 1 : 0;
}

// 正则查找：返回第一个匹配子串；无匹配返回空串。
sw_string* regex_find(sw_string* text, sw_string* pattern) {
    sw_gc_disable();
    sw_string* result = rx_regex_find_impl(text, pattern);
    sw_gc_enable();
    return result;
}

static sw_string* rx_regex_find_impl(sw_string* text, sw_string* pattern) {
    if (text == NULL || pattern == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t groups = 0;
    rx_node* node = rx_compile(pattern->data, pattern->len, &groups);
    rx_codepoints cp = rx_decode(text->data, text->len);
    int64_t start = 0;
    int64_t end = 0;
    if (rx_search(node, cp.data, cp.len, &start, &end)) {
        return rx_slice(cp.data, start, end);
    }
    return sw_string_from_literal("", 0);
}

// 正则查找全部：返回所有匹配子串（非重叠，string[]）。
sw_array* regex_find_all(sw_string* text, sw_string* pattern) {
    sw_gc_disable();
    sw_array* result = rx_regex_find_all_impl(text, pattern);
    sw_gc_enable();
    return result;
}

static sw_array* rx_regex_find_all_impl(sw_string* text, sw_string* pattern) {
    sw_array* out = sw_array_new(8, 16);
    if (text == NULL || pattern == NULL) {
        out->len = 0;
        return out;
    }
    int64_t groups = 0;
    rx_node* node = rx_compile(pattern->data, pattern->len, &groups);
    rx_codepoints cp = rx_decode(text->data, text->len);
    int64_t slot = 0;
    int64_t from = 0;
    while (from <= cp.len) {
        int64_t start = 0;
        int64_t end = 0;
        // 从 from 起查找：先尝试 from 位置匹配，否则跳过单码点。
        int64_t caps[16];
        for (int64_t i = 0; i < 16; i++) {
            caps[i] = -1;
        }
        int64_t e = rx_match_full(node, cp.data, cp.len, from, caps, 16);
        if (e >= 0) {
            start = from;
            end = e;
            if (slot >= out->len) {
                sw_array* bigger = sw_array_new(8, out->len * 2 + 1);
                for (int64_t i = 0; i < slot; i++) {
                    ((int64_t*)bigger->data)[i] = ((int64_t*)out->data)[i];
                }
                out = bigger;
            }
            ((int64_t*)out->data)[slot++] = (int64_t)rx_slice(cp.data, start, end);
            from = end > start ? end : start + 1;
        } else {
            from++;
        }
    }
    out->len = slot;
    out->cap = slot;
    return out;
}

// 正则替换：把 text 中所有匹配替换为 replacement（支持 $0 与 $1..$9 分组引用）。
sw_string* regex_replace(sw_string* text, sw_string* pattern, sw_string* replacement) {
    sw_gc_disable();
    sw_string* result = rx_regex_replace_impl(text, pattern, replacement);
    sw_gc_enable();
    return result;
}

static sw_string* rx_regex_replace_impl(sw_string* text, sw_string* pattern, sw_string* replacement) {
    if (text == NULL || pattern == NULL || replacement == NULL) {
        return text;
    }
    int64_t groups = 0;
    rx_node* node = rx_compile(pattern->data, pattern->len, &groups);
    rx_codepoints cp = rx_decode(text->data, text->len);
    // 收集所有匹配区间。
    int64_t* starts = (int64_t*)sw_gc_alloc((uint64_t)(cp.len + 1) * 8);
    int64_t* ends = (int64_t*)sw_gc_alloc((uint64_t)(cp.len + 1) * 8);
    // 每个匹配的捕获组（最多 8 组 × 起止，match_count × 16）。
    int64_t* cap_store = (int64_t*)sw_gc_alloc((uint64_t)(cp.len + 1) * 16 * 8);
    int64_t match_count = 0;
    int64_t from = 0;
    while (from <= cp.len) {
        int64_t caps[16];
        for (int64_t i = 0; i < 16; i++) {
            caps[i] = -1;
        }
        int64_t e = rx_match_full(node, cp.data, cp.len, from, caps, 16);
        if (e >= 0) {
            starts[match_count] = from;
            ends[match_count] = e;
            for (int64_t k = 0; k < 16; k++) {
                cap_store[match_count * 16 + k] = caps[k];
            }
            match_count++;
            from = e > from ? e : from + 1;
        } else {
            from++;
        }
    }
    // 拼接：原文本 + 替换。
    // 先计算输出字节容量（上限：原文 + 替换 × 匹配数）。
    int64_t cap = text->len + replacement->len * match_count + 16;
    char* buffer = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t used = 0;
    // 拷贝原文码点 → 字节，需要把替换片段也转为字节。
    char* text_bytes = (char*)sw_gc_alloc((uint64_t)(text->len + 1));
    for (int64_t i = 0; i < text->len; i++) {
        text_bytes[i] = text->data[i];
    }
    text_bytes[text->len] = 0;
    int64_t cursor = 0;
    for (int64_t m = 0; m < match_count; m++) {
        int64_t start = starts[m];
        int64_t end = ends[m];
        int64_t* caps = cap_store + m * 16;
        // 拷贝匹配前原文：码点 [cursor, start) → 字节。
        sw_string* prefix = rx_slice(cp.data, cursor, start);
        for (int64_t i = 0; i < prefix->len && used + 1 < cap; i++) {
            buffer[used++] = prefix->data[i];
        }
        // 解析 replacement 中的 $N。
        for (int64_t i = 0; i < replacement->len; i++) {
            if (replacement->data[i] == '$' && i + 1 < replacement->len) {
                int64_t n = replacement->data[i + 1];
                if (n >= '0' && n <= '9') {
                    int64_t idx = n - '0';
                    if (idx == 0) {
                        sw_string* whole = rx_slice(cp.data, start, end);
                        for (int64_t k = 0; k < whole->len && used + 1 < cap; k++) {
                            buffer[used++] = whole->data[k];
                        }
                    } else {
                        int64_t cs = idx * 2;
                        int64_t ce = idx * 2 + 1;
                        if (cs < 16 && ce < 16 && caps[cs] >= 0 && caps[ce] >= caps[cs]) {
                            sw_string* group = rx_slice(cp.data, caps[cs], caps[ce]);
                            for (int64_t k = 0; k < group->len && used + 1 < cap; k++) {
                                buffer[used++] = group->data[k];
                            }
                        } else {
                            // 分组未参与匹配：按字面 $N 输出。
                            if (used + 1 < cap) {
                                buffer[used++] = replacement->data[i];
                            }
                            if (used + 1 < cap) {
                                buffer[used++] = replacement->data[i + 1];
                            }
                        }
                    }
                    i++;
                    continue;
                }
            }
            if (used + 1 < cap) {
                buffer[used++] = replacement->data[i];
            }
        }
        cursor = end;
    }
    // 尾部原文。
    sw_string* tail = rx_slice(cp.data, cursor, cp.len);
    for (int64_t i = 0; i < tail->len && used + 1 < cap; i++) {
        buffer[used++] = tail->data[i];
    }
    buffer[used] = 0;
    return sw_string_from_literal(buffer, used);
}

// ---------------------------------------------------------------------------
// 正则增强（std/regex）：拆分 / 转义 / 捕获组。
// ---------------------------------------------------------------------------

// 从 from 起搜索第一个匹配（rx_search 只能从头搜）。
static int64_t rx_search_from(
    rx_node* node, int64_t* text, int64_t len, int64_t from, int64_t* out_start, int64_t* out_end
) {
    for (int64_t s = from; s <= len; s++) {
        int64_t caps[16];
        for (int64_t i = 0; i < 16; i++) {
            caps[i] = -1;
        }
        int64_t e = rx_match_full(node, text, len, s, caps, 16);
        if (e >= 0) {
            *out_start = s;
            *out_end = e;
            return 1;
        }
    }
    return 0;
}

// 按正则匹配位置拆分：regex_split("a,b;c", "[,;]") == ["a","b","c"]。
sw_array* regex_split(sw_string* text, sw_string* pattern) {
    sw_gc_disable();
    sw_array* result = rx_regex_split_impl(text, pattern);
    sw_gc_enable();
    return result;
}

static sw_array* rx_regex_split_impl(sw_string* text, sw_string* pattern) {
    sw_array* out = sw_array_new(8, 16);
    if (text == NULL || pattern == NULL) {
        out->len = 0;
        return out;
    }
    int64_t groups = 0;
    rx_node* node = rx_compile(pattern->data, pattern->len, &groups);
    rx_codepoints cp = rx_decode(text->data, text->len);
    int64_t slot = 0;
    int64_t cursor = 0;
    while (cursor <= cp.len) {
        int64_t start = 0;
        int64_t end = 0;
        if (!rx_search_from(node, cp.data, cp.len, cursor, &start, &end) || start < cursor) {
            break;
        }
        if (slot >= out->len) {
            sw_array* bigger = sw_array_new(8, out->len * 2 + 1);
            for (int64_t i = 0; i < slot; i++) {
                ((int64_t*)bigger->data)[i] = ((int64_t*)out->data)[i];
            }
            out = bigger;
        }
        ((int64_t*)out->data)[slot++] = (int64_t)rx_slice(cp.data, cursor, start);
        cursor = end > start ? end : start + 1;
    }
    if (slot >= out->len) {
        sw_array* bigger = sw_array_new(8, out->len * 2 + 1);
        for (int64_t i = 0; i < slot; i++) {
            ((int64_t*)bigger->data)[i] = ((int64_t*)out->data)[i];
        }
        out = bigger;
    }
    ((int64_t*)out->data)[slot++] = (int64_t)rx_slice(cp.data, cursor, cp.len);
    out->len = slot;
    out->cap = slot;
    return out;
}

// 转义正则元字符（\. ^ $ * + ? { } [ ] ( ) | 前加反斜杠）。
sw_string* regex_escape(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)text->len * 2 + 1);
    int64_t used = 0;
    for (int64_t i = 0; i < text->len; i++) {
        char c = text->data[i];
        if (c == '.' || c == '^' || c == '$' || c == '*' || c == '+' || c == '?' ||
            c == '{' || c == '}' || c == '[' || c == ']' || c == '(' || c == ')' ||
            c == '|' || c == '\\') {
            buffer[used++] = '\\';
        }
        buffer[used++] = c;
    }
    buffer[used] = 0;
    return sw_string_from_literal(buffer, used);
}

// 提取第一个匹配的捕获组（string[]，[0] 是整个匹配，[1..] 各组；
// 未参与匹配的组返回空串）。
sw_array* regex_captures(sw_string* text, sw_string* pattern) {
    sw_gc_disable();
    sw_array* result = rx_regex_captures_impl(text, pattern);
    sw_gc_enable();
    return result;
}

static sw_array* rx_regex_captures_impl(sw_string* text, sw_string* pattern) {
    sw_array* out = sw_array_new(8, 8);
    if (text == NULL || pattern == NULL) {
        out->len = 0;
        return out;
    }
    int64_t groups = 0;
    rx_node* node = rx_compile(pattern->data, pattern->len, &groups);
    rx_codepoints cp = rx_decode(text->data, text->len);
    int64_t start = 0;
    int64_t end = 0;
    if (!rx_search(node, cp.data, cp.len, &start, &end)) {
        // 无匹配：返回与组数等长的空串数组（文档承诺"未参与匹配的组返回空串"），
        // 避免调用方访问空数组元素得到 NULL 字符串。
        int64_t count = groups > 8 ? 8 : groups;
        for (int64_t g = 0; g < count; g++) {
            ((int64_t*)out->data)[g] = (int64_t)sw_string_from_literal("", 0);
        }
        out->len = count;
        out->cap = count;
        return out;
    }
    int64_t caps[16];
    for (int64_t i = 0; i < 16; i++) {
        caps[i] = -1;
    }
    rx_match_full(node, cp.data, cp.len, start, caps, 16);
    // 引擎语义：组 0（caps[0..1]）= 整个匹配，括号组从 1 起（caps[2..]）；
    // groups = 1 + 括号数。返回 [整个匹配, 组1, 组2, ...]。
    int64_t count = groups;
    if (count > 8) {
        count = 8;
    }
    ((int64_t*)out->data)[0] = (int64_t)rx_slice(cp.data, start, end);
    for (int64_t g = 1; g < groups && g < count; g++) {
        int64_t s = caps[g * 2];
        int64_t e = caps[g * 2 + 1];
        sw_string* part = (s >= 0 && e >= s) ? rx_slice(cp.data, s, e) : sw_string_from_literal("", 0);
        ((int64_t*)out->data)[g] = (int64_t)part;
    }
    out->len = count;
    out->cap = count;
    return out;
}

// ---------------------------------------------------------------------------
// MD5 / SHA-256（标准实现，纯自包含，用于文件/文本校验）
// ---------------------------------------------------------------------------

typedef struct {
    unsigned int state[4];
    unsigned int count[2];
    unsigned char buffer[64];
} sw_md5_ctx;

static const unsigned int sw_md5_k[64] = {
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee,
    0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
    0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
    0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
    0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
    0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
    0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039,
    0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
    0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391
};

static const unsigned char sw_md5_shift[64] = {
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21
};

static unsigned int sw_md5_rotl(unsigned int value, int shift) {
    return (value << shift) | (value >> (32 - shift));
}

static void sw_md5_init(sw_md5_ctx* ctx) {
    ctx->state[0] = 0x67452301;
    ctx->state[1] = 0xefcdab89;
    ctx->state[2] = 0x98badcfe;
    ctx->state[3] = 0x10325476;
    ctx->count[0] = 0;
    ctx->count[1] = 0;
}

static void sw_md5_transform(sw_md5_ctx* ctx, const unsigned char block[64]) {
    unsigned int a = ctx->state[0];
    unsigned int b = ctx->state[1];
    unsigned int c = ctx->state[2];
    unsigned int d = ctx->state[3];
    unsigned int x[16];
    for (int i = 0; i < 16; i++) {
        x[i] = (unsigned int)block[i * 4] |
               ((unsigned int)block[i * 4 + 1] << 8) |
               ((unsigned int)block[i * 4 + 2] << 16) |
               ((unsigned int)block[i * 4 + 3] << 24);
    }
    for (int i = 0; i < 64; i++) {
        unsigned int f;
        int g;
        if (i < 16) {
            f = (b & c) | ((~b) & d);
            g = i;
        } else if (i < 32) {
            f = (d & b) | ((~d) & c);
            g = (5 * i + 1) % 16;
        } else if (i < 48) {
            f = b ^ c ^ d;
            g = (3 * i + 5) % 16;
        } else {
            f = c ^ (b | (~d));
            g = (7 * i) % 16;
        }
        unsigned int temp = d;
        d = c;
        c = b;
        b = b + sw_md5_rotl(a + f + sw_md5_k[i] + x[g], sw_md5_shift[i]);
        a = temp;
    }
    ctx->state[0] += a;
    ctx->state[1] += b;
    ctx->state[2] += c;
    ctx->state[3] += d;
}

static void sw_md5_update(sw_md5_ctx* ctx, const unsigned char* data, uint64_t len) {
    uint64_t index = (ctx->count[0] >> 3) & 0x3F;
    uint64_t add = 64 - index;
    ctx->count[0] += (unsigned int)(len << 3);
    if (ctx->count[0] < (unsigned int)(len << 3)) {
        ctx->count[1]++;
    }
    ctx->count[1] += (unsigned int)(len >> 29);
    if (len >= add) {
        memcpy(ctx->buffer + index, data, add);
        sw_md5_transform(ctx, ctx->buffer);
        for (uint64_t i = add; i + 63 < len; i += 64) {
            sw_md5_transform(ctx, data + i);
        }
        index = 0;
    } else {
        add = len;
    }
    memcpy(ctx->buffer + index, data, add);
}

static void sw_md5_final(sw_md5_ctx* ctx, unsigned char digest[16]) {
    unsigned char bits[8];
    for (int i = 0; i < 8; i++) {
        bits[i] = (unsigned char)((ctx->count[i >> 2] >> ((i & 3) * 8)) & 0xFF);
    }
    unsigned char pad[72];
    uint64_t index = (ctx->count[0] >> 3) & 0x3F;
    uint64_t pad_len = index < 56 ? 56 - index : 120 - index;
    memset(pad, 0, sizeof(pad));
    pad[0] = 0x80;
    sw_md5_update(ctx, pad, pad_len);
    sw_md5_update(ctx, bits, 8);
    for (int i = 0; i < 4; i++) {
        digest[i * 4] = (unsigned char)(ctx->state[i] & 0xFF);
        digest[i * 4 + 1] = (unsigned char)((ctx->state[i] >> 8) & 0xFF);
        digest[i * 4 + 2] = (unsigned char)((ctx->state[i] >> 16) & 0xFF);
        digest[i * 4 + 3] = (unsigned char)((ctx->state[i] >> 24) & 0xFF);
    }
}

static sw_string* sw_md5_bytes(const unsigned char* data, uint64_t len) {
    sw_md5_ctx ctx;
    sw_md5_init(&ctx);
    sw_md5_update(&ctx, data, len);
    unsigned char digest[16];
    sw_md5_final(&ctx, digest);
    char* out = (char*)sw_gc_alloc(33);
    static const char hex[] = "0123456789abcdef";
    for (int i = 0; i < 16; i++) {
        out[i * 2] = hex[digest[i] >> 4];
        out[i * 2 + 1] = hex[digest[i] & 0x0F];
    }
    out[32] = 0;
    return sw_string_from_literal(out, 32);
}

sw_string* md5(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    return sw_md5_bytes((const unsigned char*)text->data, (uint64_t)text->len);
}

sw_string* md5_file(sw_string* path) {
    if (path == NULL) {
        return sw_string_from_literal("", 0);
    }
    sw_file_handle* file = fopen(path->data, "rb");
    if (file == NULL) {
        return sw_string_from_literal("", 0);
    }
    sw_md5_ctx ctx;
    sw_md5_init(&ctx);
    unsigned char buffer[4096];
    uint64_t got;
    while ((got = fread(buffer, 1, sizeof(buffer), file)) > 0) {
        sw_md5_update(&ctx, buffer, got);
    }
    fclose(file);
    unsigned char digest[16];
    sw_md5_final(&ctx, digest);
    char* out = (char*)sw_gc_alloc(33);
    static const char hex[] = "0123456789abcdef";
    for (int i = 0; i < 16; i++) {
        out[i * 2] = hex[digest[i] >> 4];
        out[i * 2 + 1] = hex[digest[i] & 0x0F];
    }
    out[32] = 0;
    return sw_string_from_literal(out, 32);
}

// SHA-256
static const unsigned int sw_sha256_k[64] = {
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
    0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
    0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
};

typedef struct {
    unsigned int state[8];
    uint64_t count;
    unsigned char buffer[64];
} sw_sha256_ctx;

static unsigned int sw_sha256_rotr(unsigned int value, int shift) {
    return (value >> shift) | (value << (32 - shift));
}

static void sw_sha256_transform(sw_sha256_ctx* ctx, const unsigned char block[64]) {
    unsigned int w[64];
    for (int i = 0; i < 16; i++) {
        w[i] = ((unsigned int)block[i * 4] << 24) |
               ((unsigned int)block[i * 4 + 1] << 16) |
               ((unsigned int)block[i * 4 + 2] << 8) |
               (unsigned int)block[i * 4 + 3];
    }
    for (int i = 16; i < 64; i++) {
        unsigned int s0 = sw_sha256_rotr(w[i - 15], 7) ^ sw_sha256_rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
        unsigned int s1 = sw_sha256_rotr(w[i - 2], 17) ^ sw_sha256_rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16] + s0 + w[i - 7] + s1;
    }
    unsigned int a = ctx->state[0];
    unsigned int b = ctx->state[1];
    unsigned int c = ctx->state[2];
    unsigned int d = ctx->state[3];
    unsigned int e = ctx->state[4];
    unsigned int f = ctx->state[5];
    unsigned int g = ctx->state[6];
    unsigned int h = ctx->state[7];
    for (int i = 0; i < 64; i++) {
        unsigned int s1 = sw_sha256_rotr(e, 6) ^ sw_sha256_rotr(e, 11) ^ sw_sha256_rotr(e, 25);
        unsigned int ch = (e & f) ^ ((~e) & g);
        unsigned int temp1 = h + s1 + ch + sw_sha256_k[i] + w[i];
        unsigned int s0 = sw_sha256_rotr(a, 2) ^ sw_sha256_rotr(a, 13) ^ sw_sha256_rotr(a, 22);
        unsigned int maj = (a & b) ^ (a & c) ^ (b & c);
        unsigned int temp2 = s0 + maj;
        h = g;
        g = f;
        f = e;
        e = d + temp1;
        d = c;
        c = b;
        b = a;
        a = temp1 + temp2;
    }
    ctx->state[0] += a;
    ctx->state[1] += b;
    ctx->state[2] += c;
    ctx->state[3] += d;
    ctx->state[4] += e;
    ctx->state[5] += f;
    ctx->state[6] += g;
    ctx->state[7] += h;
}

static void sw_sha256_init(sw_sha256_ctx* ctx) {
    ctx->state[0] = 0x6a09e667;
    ctx->state[1] = 0xbb67ae85;
    ctx->state[2] = 0x3c6ef372;
    ctx->state[3] = 0xa54ff53a;
    ctx->state[4] = 0x510e527f;
    ctx->state[5] = 0x9b05688c;
    ctx->state[6] = 0x1f83d9ab;
    ctx->state[7] = 0x5be0cd19;
    ctx->count = 0;
}

static void sw_sha256_update(sw_sha256_ctx* ctx, const unsigned char* data, uint64_t len) {
    uint64_t index = ctx->count & 0x3F;
    ctx->count += len;
    uint64_t add = 64 - index;
    if (len >= add) {
        memcpy(ctx->buffer + index, data, add);
        sw_sha256_transform(ctx, ctx->buffer);
        uint64_t i;
        for (i = add; i + 63 < len; i += 64) {
            sw_sha256_transform(ctx, data + i);
        }
        index = 0;
        data += i;
        len -= i;
    }
    if (len > 0) {
        memcpy(ctx->buffer + index, data, len);
    }
}

static void sw_sha256_final(sw_sha256_ctx* ctx, unsigned char digest[32]) {
    uint64_t bits = ctx->count << 3;
    unsigned char pad[72];
    uint64_t index = ctx->count & 0x3F;
    uint64_t pad_len = index < 56 ? 56 - index : 120 - index;
    memset(pad, 0, sizeof(pad));
    pad[0] = 0x80;
    sw_sha256_update(ctx, pad, pad_len);
    unsigned char len_bytes[8];
    for (int i = 0; i < 8; i++) {
        len_bytes[i] = (unsigned char)((bits >> (56 - i * 8)) & 0xFF);
    }
    sw_sha256_update(ctx, len_bytes, 8);
    for (int i = 0; i < 8; i++) {
        digest[i * 4] = (unsigned char)((ctx->state[i] >> 24) & 0xFF);
        digest[i * 4 + 1] = (unsigned char)((ctx->state[i] >> 16) & 0xFF);
        digest[i * 4 + 2] = (unsigned char)((ctx->state[i] >> 8) & 0xFF);
        digest[i * 4 + 3] = (unsigned char)(ctx->state[i] & 0xFF);
    }
}

static sw_string* sw_sha256_bytes(const unsigned char* data, uint64_t len) {
    sw_sha256_ctx ctx;
    sw_sha256_init(&ctx);
    sw_sha256_update(&ctx, data, len);
    unsigned char digest[32];
    sw_sha256_final(&ctx, digest);
    char* out = (char*)sw_gc_alloc(65);
    static const char hex[] = "0123456789abcdef";
    for (int i = 0; i < 32; i++) {
        out[i * 2] = hex[digest[i] >> 4];
        out[i * 2 + 1] = hex[digest[i] & 0x0F];
    }
    out[64] = 0;
    return sw_string_from_literal(out, 64);
}

sw_string* sha256(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    return sw_sha256_bytes((const unsigned char*)text->data, (uint64_t)text->len);
}

sw_string* sha256_file(sw_string* path) {
    if (path == NULL) {
        return sw_string_from_literal("", 0);
    }
    sw_file_handle* file = fopen(path->data, "rb");
    if (file == NULL) {
        return sw_string_from_literal("", 0);
    }
    sw_sha256_ctx ctx;
    sw_sha256_init(&ctx);
    unsigned char buffer[4096];
    uint64_t got;
    while ((got = fread(buffer, 1, sizeof(buffer), file)) > 0) {
        sw_sha256_update(&ctx, buffer, got);
    }
    fclose(file);
    unsigned char digest[32];
    sw_sha256_final(&ctx, digest);
    char* out = (char*)sw_gc_alloc(65);
    static const char hex[] = "0123456789abcdef";
    for (int i = 0; i < 32; i++) {
        out[i * 2] = hex[digest[i] >> 4];
        out[i * 2 + 1] = hex[digest[i] & 0x0F];
    }
    out[64] = 0;
    return sw_string_from_literal(out, 64);
}

// ---------------------------------------------------------------------------
// 哈希增强（std/hash）：CRC-32 / CRC-16、SHA-1、HMAC-SHA256。
// ---------------------------------------------------------------------------

// CRC-32（IEEE 802.3，反射多项式 0xEDB88320，无表逐位）。返回 uint32 位模式。
uint64_t crc32(sw_string* text) {
    uint64_t crc = 0xFFFFFFFFu;
    if (text != NULL) {
        for (int64_t i = 0; i < text->len; i++) {
            crc ^= (unsigned char)text->data[i];
            for (int bit = 0; bit < 8; bit++) {
                crc = (crc >> 1) ^ (0xEDB88320u & (0u - (crc & 1)));
            }
        }
    }
    return crc ^ 0xFFFFFFFFu;
}

uint64_t crc32_file(sw_string* path) {
    uint64_t crc = 0xFFFFFFFFu;
    if (path == NULL) {
        return 0;
    }
    sw_file_handle* file = fopen(path->data, "rb");
    if (file == NULL) {
        return 0;
    }
    unsigned char buffer[4096];
    uint64_t got;
    while ((got = fread(buffer, 1, sizeof(buffer), file)) > 0) {
        for (uint64_t i = 0; i < got; i++) {
            crc ^= buffer[i];
            for (int bit = 0; bit < 8; bit++) {
                crc = (crc >> 1) ^ (0xEDB88320u & (0u - (crc & 1)));
            }
        }
    }
    fclose(file);
    return crc ^ 0xFFFFFFFFu;
}

// CRC-16（CRC-16/IBM，反射多项式 0xA001）。返回 uint16 位模式。
uint64_t crc16(sw_string* text) {
    uint64_t crc = 0;
    if (text != NULL) {
        for (int64_t i = 0; i < text->len; i++) {
            crc ^= (unsigned char)text->data[i];
            for (int bit = 0; bit < 8; bit++) {
                crc = (crc >> 1) ^ (0xA001u & (0u - (crc & 1)));
            }
        }
    }
    return crc & 0xFFFFu;
}

// SHA-1
typedef struct {
    unsigned int state[5];
    uint64_t count;
    unsigned char buffer[64];
} sw_sha1_ctx;

static unsigned int sw_sha1_rotl(unsigned int value, int shift) {
    return (value << shift) | (value >> (32 - shift));
}

static void sw_sha1_transform(sw_sha1_ctx* ctx, const unsigned char block[64]) {
    unsigned int w[80];
    for (int i = 0; i < 16; i++) {
        w[i] = ((unsigned int)block[i * 4] << 24) |
               ((unsigned int)block[i * 4 + 1] << 16) |
               ((unsigned int)block[i * 4 + 2] << 8) |
               (unsigned int)block[i * 4 + 3];
    }
    for (int i = 16; i < 80; i++) {
        w[i] = sw_sha1_rotl(w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16], 1);
    }
    unsigned int a = ctx->state[0];
    unsigned int b = ctx->state[1];
    unsigned int c = ctx->state[2];
    unsigned int d = ctx->state[3];
    unsigned int e = ctx->state[4];
    for (int i = 0; i < 80; i++) {
        unsigned int f;
        unsigned int k;
        if (i < 20) {
            f = (b & c) | ((~b) & d);
            k = 0x5A827999u;
        } else if (i < 40) {
            f = b ^ c ^ d;
            k = 0x6ED9EBA1u;
        } else if (i < 60) {
            f = (b & c) | (b & d) | (c & d);
            k = 0x8F1BBCDCu;
        } else {
            f = b ^ c ^ d;
            k = 0xCA62C1D6u;
        }
        unsigned int temp = sw_sha1_rotl(a, 5) + f + e + k + w[i];
        e = d;
        d = c;
        c = sw_sha1_rotl(b, 30);
        b = a;
        a = temp;
    }
    ctx->state[0] += a;
    ctx->state[1] += b;
    ctx->state[2] += c;
    ctx->state[3] += d;
    ctx->state[4] += e;
}

static void sw_sha1_init(sw_sha1_ctx* ctx) {
    ctx->state[0] = 0x67452301u;
    ctx->state[1] = 0xEFCDAB89u;
    ctx->state[2] = 0x98BADCFEu;
    ctx->state[3] = 0x10325476u;
    ctx->state[4] = 0xC3D2E1F0u;
    ctx->count = 0;
}

static void sw_sha1_update(sw_sha1_ctx* ctx, const unsigned char* data, uint64_t len) {
    uint64_t index = ctx->count & 0x3F;
    ctx->count += len;
    uint64_t add = 64 - index;
    if (len >= add) {
        memcpy(ctx->buffer + index, data, add);
        sw_sha1_transform(ctx, ctx->buffer);
        uint64_t i;
        for (i = add; i + 63 < len; i += 64) {
            sw_sha1_transform(ctx, data + i);
        }
        index = 0;
        data += i;
        len -= i;
    }
    if (len > 0) {
        memcpy(ctx->buffer + index, data, len);
    }
}

static void sw_sha1_final(sw_sha1_ctx* ctx, unsigned char digest[20]) {
    uint64_t bits = ctx->count << 3;
    unsigned char pad[72];
    uint64_t index = ctx->count & 0x3F;
    uint64_t pad_len = index < 56 ? 56 - index : 120 - index;
    memset(pad, 0, sizeof(pad));
    pad[0] = 0x80;
    sw_sha1_update(ctx, pad, pad_len);
    unsigned char len_bytes[8];
    for (int i = 0; i < 8; i++) {
        len_bytes[i] = (unsigned char)((bits >> (56 - i * 8)) & 0xFF);
    }
    sw_sha1_update(ctx, len_bytes, 8);
    for (int i = 0; i < 5; i++) {
        digest[i * 4] = (unsigned char)((ctx->state[i] >> 24) & 0xFF);
        digest[i * 4 + 1] = (unsigned char)((ctx->state[i] >> 16) & 0xFF);
        digest[i * 4 + 2] = (unsigned char)((ctx->state[i] >> 8) & 0xFF);
        digest[i * 4 + 3] = (unsigned char)(ctx->state[i] & 0xFF);
    }
}

static sw_string* sw_sha1_bytes(const unsigned char* data, uint64_t len) {
    sw_sha1_ctx ctx;
    sw_sha1_init(&ctx);
    sw_sha1_update(&ctx, data, len);
    unsigned char digest[20];
    sw_sha1_final(&ctx, digest);
    char* out = (char*)sw_gc_alloc(41);
    static const char hex[] = "0123456789abcdef";
    for (int i = 0; i < 20; i++) {
        out[i * 2] = hex[digest[i] >> 4];
        out[i * 2 + 1] = hex[digest[i] & 0x0F];
    }
    out[40] = 0;
    return sw_string_from_literal(out, 40);
}

sw_string* sha1(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    return sw_sha1_bytes((const unsigned char*)text->data, (uint64_t)text->len);
}

sw_string* sha1_file(sw_string* path) {
    if (path == NULL) {
        return sw_string_from_literal("", 0);
    }
    sw_file_handle* file = fopen(path->data, "rb");
    if (file == NULL) {
        return sw_string_from_literal("", 0);
    }
    sw_sha1_ctx ctx;
    sw_sha1_init(&ctx);
    unsigned char buffer[4096];
    uint64_t got;
    while ((got = fread(buffer, 1, sizeof(buffer), file)) > 0) {
        sw_sha1_update(&ctx, buffer, got);
    }
    fclose(file);
    unsigned char digest[20];
    sw_sha1_final(&ctx, digest);
    char* out = (char*)sw_gc_alloc(41);
    static const char hex[] = "0123456789abcdef";
    for (int i = 0; i < 20; i++) {
        out[i * 2] = hex[digest[i] >> 4];
        out[i * 2 + 1] = hex[digest[i] & 0x0F];
    }
    out[40] = 0;
    return sw_string_from_literal(out, 40);
}

// HMAC-SHA256：key 超 64 字节先哈希；内层 ipad^key || message，外层 opad^key || 内层摘要。
sw_string* hmac_sha256(sw_string* key, sw_string* text) {
    unsigned char k[64];
    memset(k, 0, sizeof(k));
    if (key != NULL) {
        if (key->len > 64) {
            sw_sha256_ctx ctx;
            sw_sha256_init(&ctx);
            sw_sha256_update(&ctx, (const unsigned char*)key->data, (uint64_t)key->len);
            unsigned char digest[32];
            sw_sha256_final(&ctx, digest);
            memcpy(k, digest, 32);
        } else {
            memcpy(k, key->data, (uint64_t)key->len);
        }
    }
    unsigned char ipad[64];
    unsigned char opad[64];
    for (int i = 0; i < 64; i++) {
        ipad[i] = k[i] ^ 0x36u;
        opad[i] = k[i] ^ 0x5Cu;
    }
    sw_sha256_ctx inner;
    sw_sha256_init(&inner);
    sw_sha256_update(&inner, ipad, 64);
    if (text != NULL) {
        sw_sha256_update(&inner, (const unsigned char*)text->data, (uint64_t)text->len);
    }
    unsigned char inner_digest[32];
    sw_sha256_final(&inner, inner_digest);
    sw_sha256_ctx outer;
    sw_sha256_init(&outer);
    sw_sha256_update(&outer, opad, 64);
    sw_sha256_update(&outer, inner_digest, 32);
    unsigned char digest[32];
    sw_sha256_final(&outer, digest);
    char* out = (char*)sw_gc_alloc(65);
    static const char hex[] = "0123456789abcdef";
    for (int i = 0; i < 32; i++) {
        out[i * 2] = hex[digest[i] >> 4];
        out[i * 2 + 1] = hex[digest[i] & 0x0F];
    }
    out[64] = 0;
    return sw_string_from_literal(out, 64);
}

// 解析 URL 为 map：scheme / host / port(int) / path / query。
void* url_parse(sw_string* url) {
    void* map = sw_map_new();
    if (url == NULL) {
        return map;
    }
    // scheme://
    int64_t scheme_end = -1;
    for (int64_t i = 0; i + 2 < url->len; i++) {
        if (url->data[i] == ':' && url->data[i + 1] == '/' && url->data[i + 2] == '/') {
            scheme_end = i;
            break;
        }
    }
    int64_t host_start = 0;
    sw_string* scheme = sw_string_from_literal("", 0);
    if (scheme_end >= 0) {
        scheme = sw_string_from_literal(url->data, scheme_end);
        host_start = scheme_end + 3;
    }
    // host 部分：到 / ? # 或结尾
    int64_t host_end = host_start;
    while (host_end < url->len) {
        char c = url->data[host_end];
        if (c == '/' || c == '?' || c == '#') {
            break;
        }
        host_end++;
    }
    sw_string* host = sw_string_from_literal(url->data + host_start, host_end - host_start);
    // 端口
    int64_t port = 0;
    for (int64_t i = host_start; i < host_end; i++) {
        if (url->data[i] == ':') {
            int64_t p = 0;
            for (int64_t k = i + 1; k < host_end; k++) {
                if (url->data[k] < '0' || url->data[k] > '9') {
                    break;
                }
                p = p * 10 + (url->data[k] - '0');
            }
            host = sw_string_from_literal(url->data + host_start, i - host_start);
            port = p;
            break;
        }
    }
    if (port == 0) {
        if (scheme->len == 5 && scheme->data[0] == 'h' && scheme->data[1] == 't' &&
            scheme->data[2] == 't' && scheme->data[3] == 'p' && scheme->data[4] == 's') {
            port = 443;
        } else {
            port = 80;
        }
    }
    // path 与 query
    int64_t path_start = host_end;
    int64_t query_start = -1;
    for (int64_t i = path_start; i < url->len; i++) {
        if (url->data[i] == '?') {
            query_start = i;
            break;
        }
        if (url->data[i] == '#') {
            query_start = i;
            break;
        }
    }
    int64_t path_end = query_start >= 0 ? query_start : url->len;
    sw_string* path = path_start < path_end
        ? sw_string_from_literal(url->data + path_start, path_end - path_start)
        : sw_string_from_literal("/", 1);
    sw_string* query = query_start >= 0 && query_start + 1 < url->len
        ? sw_string_from_literal(url->data + query_start + 1, url->len - query_start - 1)
        : sw_string_from_literal("", 0);
    sw_map_set(map, sw_string_from_literal("scheme", 6), scheme);
    sw_map_set(map, sw_string_from_literal("host", 4), host);
    sw_map_set_int(map, sw_string_from_literal("port", 4), port);
    sw_map_set(map, sw_string_from_literal("path", 4), path);
    sw_map_set(map, sw_string_from_literal("query", 5), query);
    return map;
}

// 解析查询字符串 "a=1&b=2" 为 map（string 值，键值自动 URL 解码；
// '+' 视为空格，符合 application/x-www-form-urlencoded）。
void* url_query(sw_string* query) {
    void* map = sw_map_new();
    if (query == NULL) {
        return map;
    }
    int64_t i = 0;
    while (i <= query->len) {
        int64_t seg_end = i;
        while (seg_end < query->len && query->data[seg_end] != '&') {
            seg_end++;
        }
        if (seg_end > i) {
            int64_t eq = i;
            while (eq < seg_end && query->data[eq] != '=') {
                eq++;
            }
            sw_string* key = eq < seg_end
                ? sw_string_from_literal(query->data + i, eq - i)
                : sw_string_from_literal(query->data + i, seg_end - i);
            sw_string* value = eq < seg_end && eq + 1 < seg_end
                ? sw_string_from_literal(query->data + eq + 1, seg_end - eq - 1)
                : sw_string_from_literal("", 0);
            // 先 '+' -> 空格，再整体百分号解码
            sw_string* key_spaced = sw_string_from_literal(key->data, key->len);
            sw_string* value_spaced = sw_string_from_literal(value->data, value->len);
            for (int64_t k = 0; k < key_spaced->len; k++) {
                if (key_spaced->data[k] == '+') {
                    key_spaced->data[k] = ' ';
                }
            }
            for (int64_t k = 0; k < value_spaced->len; k++) {
                if (value_spaced->data[k] == '+') {
                    value_spaced->data[k] = ' ';
                }
            }
            sw_map_set(map, sw_url_decode(key_spaced), sw_url_decode(value_spaced));
        }
        i = seg_end + 1;
    }
    return map;
}

// 把 map 序列化为查询字符串 "a=1&b=2"（URL 编码键与值，动态扩容不截断）。
sw_string* url_build_query(void* map) {
    sw_array* keys = sw_map_keys(map);
    sw_array* values = sw_map_values(map);
    int64_t* kdata = (int64_t*)keys->data;
    int64_t* vdata = (int64_t*)values->data;
    int64_t cap = 256;
    char* buffer = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t used = 0;
    for (int64_t i = 0; i < keys->len; i++) {
        sw_string* key = (sw_string*)kdata[i];
        sw_string* value = (sw_string*)vdata[i];
        if (i > 0) {
            if (used + 1 >= cap) {
                cap *= 2;
                buffer = (char*)sw_gc_alloc((uint64_t)cap);
            }
            buffer[used++] = '&';
        }
        for (int64_t k = 0; k < key->len; k++) {
            unsigned char c = (unsigned char)key->data[k];
            if ((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
                (c >= '0' && c <= '9') || c == '-' || c == '_' || c == '.') {
                if (used + 1 >= cap) {
                    cap *= 2;
                    buffer = (char*)sw_gc_alloc((uint64_t)cap);
                }
                buffer[used++] = (char)c;
            } else {
                if (used + 3 >= cap) {
                    cap *= 2;
                    buffer = (char*)sw_gc_alloc((uint64_t)cap);
                }
                used += snprintf(buffer + used, (sw_size)(cap - used), "%%%02X", c);
            }
        }
        if (used + 1 >= cap) {
            cap *= 2;
            buffer = (char*)sw_gc_alloc((uint64_t)cap);
        }
        buffer[used++] = '=';
        for (int64_t k = 0; k < value->len; k++) {
            unsigned char c = (unsigned char)value->data[k];
            if ((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
                (c >= '0' && c <= '9') || c == '-' || c == '_' || c == '.') {
                if (used + 1 >= cap) {
                    cap *= 2;
                    buffer = (char*)sw_gc_alloc((uint64_t)cap);
                }
                buffer[used++] = (char)c;
            } else {
                if (used + 3 >= cap) {
                    cap *= 2;
                    buffer = (char*)sw_gc_alloc((uint64_t)cap);
                }
                used += snprintf(buffer + used, (sw_size)(cap - used), "%%%02X", c);
            }
        }
    }
    buffer[used] = 0;
    return sw_string_from_literal(buffer, used);
}

// ---------------------------------------------------------------------------
// HTTP 客户端（阻塞式，基于 sw_net_*）
// 返回 map：status(int) / body(string) / headers(string)。
// ---------------------------------------------------------------------------

static int64_t sw_http_parse_status(sw_string* text) {
    // 期望 "HTTP/1.1 200 OK" 或 "HTTP/1.0 404 ..."
    int64_t space = -1;
    for (int64_t i = 0; i < text->len; i++) {
        if (text->data[i] == ' ') {
            space = i;
            break;
        }
    }
    if (space < 0 || space + 1 >= text->len) {
        return 0;
    }
    int64_t code = 0;
    int64_t i = space + 1;
    while (i < text->len && text->data[i] >= '0' && text->data[i] <= '9') {
        code = code * 10 + (text->data[i] - '0');
        i++;
    }
    return code;
}

// 读取直到对端关闭，返回拼接后的全部响应文本。
static sw_string* sw_http_read_all(int64_t fd) {
    int64_t cap = 4096;
    char* buffer = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t used = 0;
    while (1) {
        sw_string* chunk = sw_net_recv(fd, 65536);
        if (chunk == NULL || chunk->len == 0) {
            break;
        }
        if (used + chunk->len + 1 > cap) {
            cap = (used + chunk->len) * 2 + 64;
            char* bigger = (char*)sw_gc_alloc((uint64_t)cap);
            memcpy(bigger, buffer, (uint64_t)used);
            buffer = bigger;
        }
        memcpy(buffer + used, chunk->data, (uint64_t)chunk->len);
        used += chunk->len;
    }
    buffer[used] = 0;
    return sw_string_from_literal(buffer, used);
}

// 读响应头：从响应文本中解析状态码、Content-Length、Transfer-Encoding、
// Location、Connection 等关键头，返回 header_end（\r\n\r\n 后 body 起点，
// 找不到返回 -1）。
typedef struct {
    int64_t status;
    int64_t content_length;
    int64_t chunked;       // Transfer-Encoding: chunked
    int64_t keep_close;    // Connection: close
    int64_t has_location;
    int64_t location_start;
    int64_t location_len;
} sw_http_resp_meta;

static sw_string* sw_http_trim(sw_string* text) {
    int64_t s = 0;
    int64_t e = text->len;
    while (s < e && (text->data[s] == ' ' || text->data[s] == '\t' || text->data[s] == '\r' || text->data[s] == '\n')) {
        s++;
    }
    while (e > s && (text->data[e - 1] == ' ' || text->data[e - 1] == '\t' || text->data[e - 1] == '\r' || text->data[e - 1] == '\n')) {
        e--;
    }
    return sw_string_from_literal(text->data + s, e - s);
}

static int64_t sw_http_header_value(sw_string* text, int64_t start, int64_t end, const char* name, int64_t name_len, int64_t* out_start, int64_t* out_len) {
    // 在 [start,end) 的头部区逐行找 "Name:"（大小写不敏感），返回值的起止。
    int64_t i = start;
    while (i < end) {
        int64_t line_end = i;
        while (line_end < end && text->data[line_end] != '\n') {
            line_end++;
        }
        int64_t line_len = line_end - i;
        if (line_len > 0 && text->data[line_end - 1] == '\r') {
            line_len--;
        }
        if (line_len > name_len + 1 &&
            (text->data[i + name_len] == ':') &&
            text->data[i + name_len + 1] == ' ') {
            int64_t match = 1;
            for (int64_t k = 0; k < name_len; k++) {
                char a = text->data[i + k];
                char b = name[k];
                if (a >= 'A' && a <= 'Z') {
                    a = (char)(a - 'A' + 'a');
                }
                if (b >= 'A' && b <= 'Z') {
                    b = (char)(b - 'A' + 'a');
                }
                if (a != b) {
                    match = 0;
                    break;
                }
            }
            if (match) {
                int64_t vs = i + name_len + 1;
                while (vs < i + line_len && (text->data[vs] == ' ' || text->data[vs] == '\t')) {
                    vs++;
                }
                *out_start = vs;
                *out_len = i + line_len - vs;
                return 1;
            }
        }
        i = line_end + 1;
    }
    return 0;
}

static int64_t sw_http_parse_meta(sw_string* text, sw_http_resp_meta* meta) {
    memset(meta, 0, sizeof(*meta));
    meta->content_length = -1;
    int64_t header_end = -1;
    for (int64_t i = 0; i + 3 < text->len; i++) {
        if (text->data[i] == '\r' && text->data[i + 1] == '\n' &&
            text->data[i + 2] == '\r' && text->data[i + 3] == '\n') {
            header_end = i;
            break;
        }
    }
    if (header_end < 0) {
        return -1;
    }
    meta->status = sw_http_parse_status(text);
    int64_t vs = 0;
    int64_t vl = 0;
    if (sw_http_header_value(text, 0, header_end, "content-length", 14, &vs, &vl)) {
        sw_string* v = sw_http_trim(sw_string_from_literal(text->data + vs, vl));
        int64_t n = 0;
        for (int64_t k = 0; k < v->len; k++) {
            if (v->data[k] < '0' || v->data[k] > '9') {
                break;
            }
            n = n * 10 + (v->data[k] - '0');
        }
        meta->content_length = n;
    }
    if (sw_http_header_value(text, 0, header_end, "transfer-encoding", 17, &vs, &vl)) {
        sw_string* v = sw_http_trim(sw_string_from_literal(text->data + vs, vl));
        if (v->len >= 7 && v->data[0] == 'c' && v->data[1] == 'h' && v->data[2] == 'u' &&
            v->data[3] == 'n' && v->data[4] == 'k' && v->data[5] == 'e' && v->data[6] == 'd') {
            meta->chunked = 1;
        }
    }
    if (sw_http_header_value(text, 0, header_end, "connection", 10, &vs, &vl)) {
        sw_string* v = sw_http_trim(sw_string_from_literal(text->data + vs, vl));
        if ((v->len >= 5 && v->data[0] == 'c' && v->data[1] == 'l' && v->data[2] == 'o' &&
             v->data[3] == 's' && v->data[4] == 'e')) {
            meta->keep_close = 1;
        }
    }
    if (sw_http_header_value(text, 0, header_end, "location", 8, &vs, &vl)) {
        meta->has_location = 1;
        meta->location_start = vs;
        meta->location_len = vl;
    }
    return header_end;
}

// 内存解码 chunked 编码的响应体（data 指向 chunked body 起点，len 为可用长度）。
// 返回解码后的完整 body；解析失败返回已解码部分。
static sw_string* sw_http_decode_chunked(const char* data, int64_t len) {
    int64_t cap = 256;
    char* buffer = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t used = 0;
    int64_t i = 0;
    while (i < len) {
        // 读 chunk-size 行（到 \r\n）
        int64_t line_end = i;
        while (line_end < len && !(data[line_end] == '\r' && line_end + 1 < len && data[line_end + 1] == '\n')) {
            line_end++;
        }
        if (line_end >= len) {
            break;
        }
        int64_t size = 0;
        int64_t k = i;
        while (k < line_end && data[k] != ';') {
            char c = data[k];
            int64_t v = -1;
            if (c >= '0' && c <= '9') {
                v = c - '0';
            } else if (c >= 'a' && c <= 'f') {
                v = c - 'a' + 10;
            } else if (c >= 'A' && c <= 'F') {
                v = c - 'A' + 10;
            } else {
                break;
            }
            size = size * 16 + v;
            k++;
        }
        i = line_end + 2;  // 跳过 \r\n
        if (size == 0) {
            break;
        }
        if (i + size > len) {
            size = len - i;
        }
        if (used + size + 1 > cap) {
            cap = (used + size) * 2 + 64;
            char* bigger = (char*)sw_gc_alloc((uint64_t)cap);
            memcpy(bigger, buffer, (uint64_t)used);
            buffer = bigger;
        }
        memcpy(buffer + used, data + i, (uint64_t)size);
        used += size;
        i += size;
        // 跳过 chunk 数据后的 \r\n
        if (i + 1 < len && data[i] == '\r' && data[i + 1] == '\n') {
            i += 2;
        }
    }
    buffer[used] = 0;
    return sw_string_from_literal(buffer, used);
}

// 把相对/绝对 Location 解析为绝对 URL。base 形如 "http://host[:port]/path..."。
// 仅支持 http://；https 返回 NULL（不声称 TLS 支持）。
static sw_string* sw_http_resolve_location(sw_string* base, sw_string* location) {
    if (base == NULL || location == NULL) {
        return NULL;
    }
    // 绝对 URL：http:// 直接返回；https:// 不支持
    if (location->len >= 7 && location->data[0] == 'h' && location->data[1] == 't' &&
        location->data[2] == 't' && location->data[3] == 'p' && location->data[4] == ':' &&
        location->data[5] == '/' && location->data[6] == '/') {
        return sw_string_from_literal(location->data, location->len);
    }
    if (location->len >= 8 && location->data[0] == 'h' && location->data[1] == 't' &&
        location->data[2] == 't' && location->data[3] == 'p' && location->data[4] == 's') {
        return NULL;
    }
    void* parts = url_parse(base);
    sw_string* scheme = sw_map_get(parts, sw_string_from_literal("scheme", 6));
    sw_string* host = sw_map_get(parts, sw_string_from_literal("host", 4));
    int64_t port = sw_map_get_int(parts, sw_string_from_literal("port", 4), 80);
    sw_string* path = sw_map_get(parts, sw_string_from_literal("path", 4));
    if (host == NULL || host->len == 0) {
        return NULL;
    }
    int64_t cap = 16 + host->len + (path != NULL ? path->len : 0) + location->len + 16;
    char* buffer = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t used = 0;
    const char* pre = "http://";
    for (int64_t i = 0; pre[i]; i++) {
        buffer[used++] = pre[i];
    }
    for (int64_t i = 0; i < host->len; i++) {
        buffer[used++] = host->data[i];
    }
    if (!(port == 80 || (scheme != NULL && scheme->len == 5 && port == 443))) {
        used += snprintf(buffer + used, (sw_size)(cap - used), ":%lld", (long long)port);
    }
    if (location->len > 0 && location->data[0] == '/') {
        for (int64_t i = 0; i < location->len; i++) {
            buffer[used++] = location->data[i];
        }
    } else {
        // 相对路径：取 base path 的目录部分拼接
        int64_t slash = -1;
        if (path != NULL) {
            for (int64_t i = 0; i < path->len; i++) {
                if (path->data[i] == '/') {
                    slash = i;
                }
            }
        }
        if (slash >= 0) {
            for (int64_t i = 0; i <= slash; i++) {
                buffer[used++] = path->data[i];
            }
        } else {
            buffer[used++] = '/';
        }
        for (int64_t i = 0; i < location->len; i++) {
            buffer[used++] = location->data[i];
        }
    }
    buffer[used] = 0;
    return sw_string_from_literal(buffer, used);
}

// 一次性 HTTP 请求核心：method/url/body → map：status/body/headers。
// timeout_ms <= 0 不设超时；>0 时连接与收发均设超时（毫秒）。
// 跟随重定向（最多 5 次，仅 http://；Location 相对/绝对均支持）。
void* sw_http_request_full(sw_string* method, sw_string* url, sw_string* body, int64_t timeout_ms) {
    void* result = sw_map_new();
    sw_map_set_int(result, sw_string_from_literal("status", 6), 0);
    sw_map_set(result, sw_string_from_literal("body", 4), sw_string_from_literal("", 0));
    sw_map_set(result, sw_string_from_literal("headers", 7), sw_string_from_literal("", 0));
    sw_map_set(result, sw_string_from_literal("url", 3), url != NULL ? url : sw_string_from_literal("", 0));
    if (url == NULL) {
        return result;
    }
    sw_string* current_url = url;
    for (int64_t redirect = 0; redirect <= 5; redirect++) {
        void* parts = url_parse(current_url);
        sw_string* scheme = sw_map_get(parts, sw_string_from_literal("scheme", 6));
        sw_string* host = sw_map_get(parts, sw_string_from_literal("host", 4));
        int64_t port = sw_map_get_int(parts, sw_string_from_literal("port", 4), 80);
        sw_string* path = sw_map_get(parts, sw_string_from_literal("path", 4));
        sw_string* query = sw_map_get(parts, sw_string_from_literal("query", 5));
        if (host == NULL || host->len == 0) {
            return result;
        }
        // 仅支持 http://
        if (scheme != NULL && scheme->len == 5 && scheme->data[0] == 'h' &&
            scheme->data[1] == 't' && scheme->data[2] == 't' && scheme->data[3] == 'p' &&
            scheme->data[4] == 's') {
            return result;
        }
        int64_t fd = timeout_ms > 0
            ? sw_net_connect_timeout(host, port, timeout_ms)
            : sw_net_connect(host, port);
        if (fd < 0) {
            return result;
        }
        if (timeout_ms > 0) {
            sw_net_set_recv_timeout(fd, timeout_ms);
            sw_net_set_send_timeout(fd, timeout_ms);
        }
        // 请求行与请求头
        char* request = (char*)sw_gc_alloc(4096);
        int64_t used = 0;
        for (int64_t i = 0; i < method->len && used < 1024; i++) {
            request[used++] = method->data[i];
        }
        request[used++] = ' ';
        for (int64_t i = 0; i < path->len && used < 2048; i++) {
            request[used++] = path->data[i];
        }
        if (query->len > 0) {
            request[used++] = '?';
            for (int64_t i = 0; i < query->len && used < 2048; i++) {
                request[used++] = query->data[i];
            }
        }
        request[used++] = ' ';
        request[used++] = 'H';
        request[used++] = 'T';
        request[used++] = 'T';
        request[used++] = 'P';
        request[used++] = '/';
        request[used++] = '1';
        request[used++] = '.';
        request[used++] = '1';
        request[used++] = '\r';
        request[used++] = '\n';
        const char* host_hdr = "Host: ";
        const char* conn_hdr = "Connection: close\r\n";
        const char* len_hdr = "Content-Length: ";
        for (int64_t i = 0; host_hdr[i]; i++) {
            request[used++] = host_hdr[i];
        }
        for (int64_t i = 0; i < host->len && used < 3072; i++) {
            request[used++] = host->data[i];
        }
        request[used++] = '\r';
        request[used++] = '\n';
        for (int64_t i = 0; conn_hdr[i]; i++) {
            request[used++] = conn_hdr[i];
        }
        int64_t body_len = body != NULL ? body->len : 0;
        if (body_len > 0) {
            for (int64_t i = 0; len_hdr[i]; i++) {
                request[used++] = len_hdr[i];
            }
            used += snprintf(request + used, (sw_size)(4096 - used), "%lld\r\n", (long long)body_len);
            const char* ctype = "Content-Type: application/x-www-form-urlencoded\r\n";
            for (int64_t i = 0; ctype[i] && used < 4090; i++) {
                request[used++] = ctype[i];
            }
        }
        request[used++] = '\r';
        request[used++] = '\n';
        sw_string* head = sw_string_from_literal(request, used);
        if (sw_net_send_all(fd, head) != head->len) {
            sw_net_close(fd);
            return result;
        }
        if (body_len > 0 && sw_net_send_all(fd, body) != body_len) {
            sw_net_close(fd);
            return result;
        }
        sw_string* response = sw_http_read_all(fd);
        sw_net_close(fd);
        sw_http_resp_meta meta;
        int64_t header_end = sw_http_parse_meta(response, &meta);
        sw_map_set_int(result, sw_string_from_literal("status", 6), meta.status);
        if (header_end >= 0) {
            sw_string* headers = sw_string_from_literal(response->data, header_end);
            sw_map_set(result, sw_string_from_literal("headers", 7), headers);
            // 重定向
            if (meta.has_location && (meta.status == 301 || meta.status == 302 ||
                                      meta.status == 303 || meta.status == 307 || meta.status == 308)) {
                sw_string* loc = sw_string_from_literal(
                    response->data + meta.location_start, meta.location_len);
                sw_string* next = sw_http_resolve_location(current_url, sw_http_trim(loc));
                if (next != NULL) {
                    current_url = next;
                    continue;
                }
            }
            int64_t body_start = header_end + 4;
            int64_t body_avail = response->len - body_start;
            if (body_avail > 0) {
                sw_string* body_text = NULL;
                if (meta.chunked) {
                    body_text = sw_http_decode_chunked(response->data + body_start, body_avail);
                } else if (meta.content_length >= 0) {
                    int64_t take = meta.content_length < body_avail ? meta.content_length : body_avail;
                    body_text = sw_string_from_literal(response->data + body_start, take);
                } else {
                    body_text = sw_string_from_literal(response->data + body_start, body_avail);
                }
                sw_map_set(result, sw_string_from_literal("body", 4), body_text);
            }
        }
        return result;
    }
    return result;
}

void* http_request(sw_string* method, sw_string* url, sw_string* body) {
    return sw_http_request_full(method, url, body, 0);
}

void* http_get(sw_string* url) {
    return sw_http_request_full(sw_string_from_literal("GET", 3), url, NULL, 0);
}

void* http_post(sw_string* url, sw_string* body) {
    return sw_http_request_full(sw_string_from_literal("POST", 4), url, body, 0);
}

// 带超时的一次性请求（timeout_ms 毫秒，<=0 不限时）。
void* http_get_timeout(sw_string* url, int64_t timeout_ms) {
    return sw_http_request_full(sw_string_from_literal("GET", 3), url, NULL, timeout_ms);
}

void* http_post_timeout(sw_string* url, sw_string* body, int64_t timeout_ms) {
    return sw_http_request_full(sw_string_from_literal("POST", 4), url, body, timeout_ms);
}

// ---------------------------------------------------------------------------
// HTTP keep-alive 会话（std/http）：同一连接复用多次请求。
// 句柄为 0-63 的表索引；响应按 Content-Length 分帧，连接保持；
// 响应无 Content-Length 或 Connection: close 时连接自动失效关闭。
// ---------------------------------------------------------------------------

#define SW_MAX_HTTP_CONNS 64

typedef struct sw_http_conn {
    int64_t fd;
    char* line_buf;
    int64_t line_len;
    int64_t line_cap;
    sw_string* host;
} sw_http_conn;

static sw_http_conn sw_http_conns[SW_MAX_HTTP_CONNS];

static int64_t sw_http_slot_alloc(void) {
    for (int64_t i = 0; i < SW_MAX_HTTP_CONNS; i++) {
        if (sw_http_conns[i].fd <= 0) {  // 静态表初始 fd=0，用 <=0 判空
            return i;
        }
    }
    return -1;
}

int64_t sw_http_open(sw_string* host, int64_t port) {
    if (host == NULL || port < 1 || port > 65535) {
        return -1;
    }
    int64_t fd = sw_net_connect(host, port);
    if (fd < 0) {
        return -1;
    }
    int64_t slot = sw_http_slot_alloc();
    if (slot < 0) {
        sw_net_close(fd);
        return -1;
    }
    sw_http_conn* c = &sw_http_conns[slot];
    memset(c, 0, sizeof(*c));
    c->fd = fd;
    c->host = host;
    return slot;
}

// 带连接超时的会话打开（timeout_ms 毫秒，<=0 不限时）。
int64_t sw_http_open_timeout(sw_string* host, int64_t port, int64_t timeout_ms) {
    if (host == NULL || port < 1 || port > 65535) {
        return -1;
    }
    int64_t fd = timeout_ms > 0
        ? sw_net_connect_timeout(host, port, timeout_ms)
        : sw_net_connect(host, port);
    if (fd < 0) {
        return -1;
    }
    if (timeout_ms > 0) {
        sw_net_set_recv_timeout(fd, timeout_ms);
        sw_net_set_send_timeout(fd, timeout_ms);
    }
    int64_t slot = sw_http_slot_alloc();
    if (slot < 0) {
        sw_net_close(fd);
        return -1;
    }
    sw_http_conn* c = &sw_http_conns[slot];
    memset(c, 0, sizeof(*c));
    c->fd = fd;
    c->host = host;
    return slot;
}

// 从会话读一行（\n 结尾，去 \r\n）；EOF 返回空串（缓冲残余也返回）。
static sw_string* sw_http_read_line(int64_t slot) {
    sw_http_conn* c = &sw_http_conns[slot];
    while (1) {
        for (int64_t i = 0; i < c->line_len; i++) {
            if (c->line_buf[i] == '\n') {
                int64_t len = i;
                if (len > 0 && c->line_buf[len - 1] == '\r') {
                    len--;
                }
                sw_string* result = sw_string_from_literal(c->line_buf, len);
                memmove(c->line_buf, c->line_buf + i + 1, (uint64_t)(c->line_len - i - 1));
                c->line_len -= (i + 1);
                return result;
            }
        }
        sw_string* chunk = sw_net_recv(c->fd, 4096);
        if (chunk == NULL || chunk->len == 0) {
            if (c->line_len > 0) {
                sw_string* result = sw_string_from_literal(c->line_buf, c->line_len);
                c->line_len = 0;
                return result;
            }
            return sw_string_from_literal("", 0);
        }
        if (c->line_len + chunk->len > c->line_cap) {
            int64_t new_cap = (c->line_len + chunk->len) * 2 + 64;
            char* bigger = (char*)realloc(c->line_buf, (sw_size)new_cap);
            if (bigger == NULL) {
                return sw_string_from_literal("", 0);
            }
            c->line_buf = bigger;
            c->line_cap = new_cap;
        }
        memcpy(c->line_buf + c->line_len, chunk->data, (uint64_t)chunk->len);
        c->line_len += chunk->len;
    }
}

// 精确读 n 字节（先消费行缓冲剩余，再收网络）。
static sw_string* sw_http_read_body(int64_t slot, int64_t n) {
    sw_http_conn* c = &sw_http_conns[slot];
    if (n <= 0) {
        return sw_string_from_literal("", 0);
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)n + 1);
    int64_t got = 0;
    if (c->line_len > 0) {
        int64_t take = c->line_len < n ? c->line_len : n;
        memcpy(buffer, c->line_buf, (uint64_t)take);
        memmove(c->line_buf, c->line_buf + take, (uint64_t)(c->line_len - take));
        c->line_len -= take;
        got = take;
    }
    while (got < n) {
        sw_string* chunk = sw_net_recv(c->fd, n - got);
        if (chunk == NULL || chunk->len == 0) {
            break;
        }
        memcpy(buffer + got, chunk->data, (uint64_t)chunk->len);
        got += chunk->len;
    }
    buffer[got] = 0;
    return sw_string_from_literal(buffer, got);
}

// 会话读取 chunked 编码响应体（逐块读，连接保持），返回解码后的 body。
static sw_string* sw_http_read_chunked_body(int64_t slot) {
    int64_t cap = 4096;
    char* buffer = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t used = 0;
    while (1) {
        sw_string* size_line = sw_http_read_line(slot);
        if (size_line == NULL || size_line->len == 0) {
            break;
        }
        // 解析十六进制块大小（可带 ; 扩展参数）
        int64_t size = 0;
        int64_t i = 0;
        while (i < size_line->len && size_line->data[i] != ';') {
            char ch = size_line->data[i];
            int64_t v = -1;
            if (ch >= '0' && ch <= '9') {
                v = ch - '0';
            } else if (ch >= 'a' && ch <= 'f') {
                v = ch - 'a' + 10;
            } else if (ch >= 'A' && ch <= 'F') {
                v = ch - 'A' + 10;
            } else {
                break;
            }
            size = size * 16 + v;
            i++;
        }
        if (size == 0) {
            // 末尾块：读取 trailer 直到空行
            while (1) {
                sw_string* trailer = sw_http_read_line(slot);
                if (trailer == NULL || trailer->len == 0) {
                    break;
                }
            }
            break;
        }
        // 单块过大（十六进制溢出为负或超 64MB）视为异常，终止解析
        if (size < 0 || size > 67108864) {
            break;
        }
        sw_string* data = sw_http_read_body(slot, size);
        if (used + data->len + 1 > cap) {
            cap = (used + data->len) * 2 + 64;
            char* bigger = (char*)sw_gc_alloc((uint64_t)cap);
            memcpy(bigger, buffer, (uint64_t)used);
            buffer = bigger;
        }
        memcpy(buffer + used, data->data, (uint64_t)data->len);
        used += data->len;
        // 跳过块数据后的 CRLF
        sw_string* crlf = sw_http_read_line(slot);
        (void)crlf;
    }
    buffer[used] = 0;
    return sw_string_from_literal(buffer, used);
}

// 关闭会话（释放槽）。
void sw_http_close(int64_t slot) {
    if (slot < 0 || slot >= SW_MAX_HTTP_CONNS || sw_http_conns[slot].fd <= 0) {
        return;
    }
    sw_http_conn* c = &sw_http_conns[slot];
    sw_net_close(c->fd);
    if (c->line_buf != NULL) {
        free(c->line_buf);
    }
    memset(c, 0, sizeof(*c));
    c->fd = -1;
}

// 会话请求：method/path/headers(map)/body → map：status/body/headers。
// 连接保持复用；失败/失效返回 status 0（连接已关闭，需重新 http_open）。
void* sw_http_request_on(
    int64_t slot, sw_string* method, sw_string* path, void* headers_map, sw_string* body
) {
    void* result = sw_map_new();
    sw_map_set_int(result, sw_string_from_literal("status", 6), 0);
    sw_map_set(result, sw_string_from_literal("body", 4), sw_string_from_literal("", 0));
    sw_map_set(result, sw_string_from_literal("headers", 7), sw_string_from_literal("", 0));
    if (slot < 0 || slot >= SW_MAX_HTTP_CONNS || sw_http_conns[slot].fd <= 0) {
        return result;
    }
    sw_http_conn* c = &sw_http_conns[slot];
    if (method == NULL || path == NULL || c->host == NULL) {
        return result;
    }
    sw_string* current_path = path;
    // 同 host 重定向跟随（最多 5 次；仅 http://，跨 host 不跟随）
    for (int64_t redirect = 0; redirect <= 5; redirect++) {
        // 构造请求头
        int64_t cap = method->len + current_path->len + c->host->len + 256;
        char* request = (char*)sw_gc_alloc((uint64_t)cap);
        int64_t used = 0;
        for (int64_t i = 0; i < method->len && used + 1 < cap; i++) {
            request[used++] = method->data[i];
        }
        request[used++] = ' ';
        for (int64_t i = 0; i < current_path->len && used + 1 < cap; i++) {
            request[used++] = current_path->data[i];
        }
        const char* proto = " HTTP/1.1\r\nHost: ";
        for (int64_t i = 0; proto[i] && used + 1 < cap; i++) {
            request[used++] = proto[i];
        }
        for (int64_t i = 0; i < c->host->len && used + 1 < cap; i++) {
            request[used++] = c->host->data[i];
        }
        const char* conn_hdr = "\r\nConnection: keep-alive\r\n";
        for (int64_t i = 0; conn_hdr[i] && used + 1 < cap; i++) {
            request[used++] = conn_hdr[i];
        }
        // 自定义请求头（map：Key → 值字符串）
        if (headers_map != NULL) {
            sw_array* keys = sw_map_keys(headers_map);
            sw_array* values = sw_map_values(headers_map);
            for (int64_t i = 0; i < keys->len && used + 2 < cap; i++) {
                sw_string* key = (sw_string*)((int64_t*)keys->data)[i];
                sw_string* value = (sw_string*)((int64_t*)values->data)[i];
                for (int64_t k = 0; k < key->len && used + 1 < cap; k++) {
                    request[used++] = key->data[k];
                }
                request[used++] = ':';
                request[used++] = ' ';
                for (int64_t k = 0; k < value->len && used + 1 < cap; k++) {
                    request[used++] = value->data[k];
                }
                request[used++] = '\r';
                request[used++] = '\n';
            }
        }
        int64_t body_len = body != NULL ? body->len : 0;
        if (body_len > 0) {
            const char* len_hdr = "Content-Length: ";
            for (int64_t i = 0; len_hdr[i] && used + 16 < cap; i++) {
                request[used++] = len_hdr[i];
            }
            used += snprintf(request + used, (sw_size)(cap - used), "%lld\r\n", (long long)body_len);
            const char* ctype = "Content-Type: application/x-www-form-urlencoded\r\n";
            for (int64_t i = 0; ctype[i] && used + 1 < cap; i++) {
                request[used++] = ctype[i];
            }
        }
        request[used++] = '\r';
        request[used++] = '\n';
        sw_string* head = sw_string_from_literal(request, used);
        if (sw_net_send_all(c->fd, head) != head->len) {
            sw_http_close(slot);
            return result;
        }
        if (body_len > 0 && sw_net_send_all(c->fd, body) != body_len) {
            sw_http_close(slot);
            return result;
        }
        // 读状态行与响应头
        sw_string* status_line = sw_http_read_line(slot);
        int64_t status = sw_http_parse_status(status_line);
        sw_map_set_int(result, sw_string_from_literal("status", 6), status);
        int64_t content_length = -1;
        int64_t chunked = 0;
        int keep = 1;
        int64_t location_start = -1;
        int64_t location_len = 0;
        int64_t header_cap = 256;
        char* header_buf = (char*)sw_gc_alloc((uint64_t)header_cap);
        int64_t header_used = 0;
        while (1) {
            sw_string* line = sw_http_read_line(slot);
            if (line->len == 0) {
                break;
            }
            if (header_used + line->len + 3 > header_cap) {
                header_cap = (header_used + line->len) * 2 + 64;
                char* bigger = (char*)sw_gc_alloc((uint64_t)header_cap);
                memcpy(bigger, header_buf, (uint64_t)header_used);
                header_buf = bigger;
            }
            memcpy(header_buf + header_used, line->data, (uint64_t)line->len);
            header_used += line->len;
            header_buf[header_used++] = '\r';
            header_buf[header_used++] = '\n';
            // Content-Length（找 ':' 再跳过空格取数字，不依赖固定偏移）
            if (line->len >= 14 && line->data[0] == 'C' && line->data[1] == 'o' &&
                line->data[2] == 'n' && line->data[3] == 't' && line->data[4] == 'e' &&
                line->data[5] == 'n' && line->data[6] == 't' && line->data[7] == '-' &&
                line->data[8] == 'L' && line->data[9] == 'e' && line->data[10] == 'n') {
                int64_t colon = -1;
                for (int64_t k = 0; k < line->len; k++) {
                    if (line->data[k] == ':') {
                        colon = k;
                        break;
                    }
                }
                int64_t v = 0;
                if (colon >= 0) {
                    int64_t i = colon + 1;
                    while (i < line->len && (line->data[i] == ' ' || line->data[i] == '\t')) {
                        i++;
                    }
                    while (i < line->len && line->data[i] >= '0' && line->data[i] <= '9') {
                        v = v * 10 + (line->data[i] - '0');
                        i++;
                    }
                }
                content_length = v;
            }
            // Transfer-Encoding: chunked
            if (line->len >= 17 && line->data[0] == 'T' && line->data[1] == 'r' &&
                line->data[2] == 'a' && line->data[3] == 'n' && line->data[4] == 's' &&
                line->data[5] == 'f' && line->data[6] == 'e' && line->data[7] == 'r' &&
                line->data[8] == '-' && line->data[9] == 'E' && line->data[10] == 'n' &&
                line->data[11] == 'c' && line->data[12] == 'o' && line->data[13] == 'd' &&
                line->data[14] == 'i' && line->data[15] == 'n' && line->data[16] == 'g') {
                int64_t colon = -1;
                for (int64_t k = 0; k < line->len; k++) {
                    if (line->data[k] == ':') {
                        colon = k;
                        break;
                    }
                }
                if (colon >= 0) {
                    for (int64_t k = colon + 1; k < line->len; k++) {
                        char ch = line->data[k];
                        if (ch == 'c' || ch == 'C') {
                            if (k + 6 < line->len && line->data[k + 1] == 'h' && line->data[k + 2] == 'u' &&
                                line->data[k + 3] == 'n' && line->data[k + 4] == 'k' &&
                                line->data[k + 5] == 'e' && line->data[k + 6] == 'd') {
                                chunked = 1;
                            }
                            break;
                        }
                    }
                }
            }
            // Location（重定向）
            if (line->len >= 8 && (line->data[0] == 'L' || line->data[0] == 'l') &&
                line->data[1] == 'o' && line->data[2] == 'c' && line->data[3] == 'a' &&
                line->data[4] == 't' && line->data[5] == 'i' && line->data[6] == 'o' &&
                line->data[7] == 'n') {
                int64_t colon = -1;
                for (int64_t k = 0; k < line->len; k++) {
                    if (line->data[k] == ':') {
                        colon = k;
                        break;
                    }
                }
                if (colon >= 0) {
                    int64_t s = colon + 1;
                    while (s < line->len && (line->data[s] == ' ' || line->data[s] == '\t')) {
                        s++;
                    }
                    location_start = s;
                    location_len = line->len - s;
                }
            }
            // Connection: close
            if (line->len >= 9 && (line->data[0] == 'C' || line->data[0] == 'c') &&
                line->data[1] == 'o' && line->data[2] == 'n' && line->data[3] == 'n') {
                sw_string* lower = line;
                for (int64_t i = 8; i + 4 < line->len; i++) {
                    if ((line->data[i] == 'c' || line->data[i] == 'C') &&
                        (line->data[i + 1] == 'l' || line->data[i + 1] == 'L') &&
                        (line->data[i + 2] == 'o' || line->data[i + 2] == 'O') &&
                        (line->data[i + 3] == 's' || line->data[i + 3] == 'S') &&
                        (line->data[i + 4] == 'e' || line->data[i + 4] == 'E')) {
                        keep = 0;
                        break;
                    }
                    (void)lower;
                }
            }
        }
        header_buf[header_used] = 0;
        sw_map_set(result, sw_string_from_literal("headers", 7), sw_string_from_literal(header_buf, header_used));
        // 重定向判断：3xx + Location，且连接仍保持时尝试同 host 跟随
        if (keep && location_start >= 0 &&
            (status == 301 || status == 302 || status == 303 || status == 307 || status == 308)) {
            sw_string* loc = sw_string_from_literal(
                header_buf + location_start, location_len);
            // 先消费当前响应体（丢弃），保持连接分帧一致
            if (chunked) {
                sw_string* discard = sw_http_read_chunked_body(slot);
                (void)discard;
            } else if (content_length >= 0) {
                sw_string* discard = sw_http_read_body(slot, content_length);
                (void)discard;
            }
            // 构造 base URL 解析 Location（同 host 才跟随）
            int64_t base_cap = c->host->len + current_path->len + 32;
            char* base_buf = (char*)sw_gc_alloc((uint64_t)base_cap);
            int64_t base_used = 0;
            const char* bp = "http://";
            for (int64_t i = 0; bp[i]; i++) {
                base_buf[base_used++] = bp[i];
            }
            for (int64_t i = 0; i < c->host->len; i++) {
                base_buf[base_used++] = c->host->data[i];
            }
            for (int64_t i = 0; i < current_path->len && base_used + 1 < base_cap; i++) {
                base_buf[base_used++] = current_path->data[i];
            }
            base_buf[base_used] = 0;
            sw_string* base_url = sw_string_from_literal(base_buf, base_used);
            sw_string* next = sw_http_resolve_location(base_url, sw_http_trim(loc));
            if (next != NULL) {
                void* next_parts = url_parse(next);
                sw_string* next_host = sw_map_get(next_parts, sw_string_from_literal("host", 4));
                int64_t next_port = sw_map_get_int(next_parts, sw_string_from_literal("port", 4), 80);
                if (next_host != NULL && string_eq(next_host, c->host) && next_port == 80) {
                    sw_string* next_path = sw_map_get(next_parts, sw_string_from_literal("path", 4));
                    sw_string* next_query = sw_map_get(next_parts, sw_string_from_literal("query", 5));
                    int64_t pcap = next_path->len + next_query->len + 2;
                    char* pbuf = (char*)sw_gc_alloc((uint64_t)pcap);
                    int64_t pu = 0;
                    for (int64_t i = 0; i < next_path->len; i++) {
                        pbuf[pu++] = next_path->data[i];
                    }
                    if (next_query->len > 0) {
                        pbuf[pu++] = '?';
                        for (int64_t i = 0; i < next_query->len; i++) {
                            pbuf[pu++] = next_query->data[i];
                        }
                    }
                    pbuf[pu] = 0;
                    current_path = sw_string_from_literal(pbuf, pu);
                    continue;
                }
            }
        }
        // 响应体：chunked → 逐块读（连接保持）；Content-Length → 精确读；
        // 否则读到关闭（连接失效）。
        if (chunked) {
            sw_string* body_text = sw_http_read_chunked_body(slot);
            sw_map_set(result, sw_string_from_literal("body", 4), body_text);
        } else if (content_length >= 0) {
            sw_string* body_text = sw_http_read_body(slot, content_length);
            sw_map_set(result, sw_string_from_literal("body", 4), body_text);
        } else {
            sw_string* body_text = sw_http_read_all(c->fd);
            sw_map_set(result, sw_string_from_literal("body", 4), body_text);
            keep = 0;
        }
        if (!keep) {
            sw_http_close(slot);
        }
        return result;
    }
    return result;
}

// ---------------------------------------------------------------------------
// 第二批：随机增强 / 格式化 / 字符串实用 / CSV / 时间 / 文件补充
// ---------------------------------------------------------------------------

// [min, max) 范围内的随机整数。
int64_t rand_int_range(int64_t min, int64_t max) {
    if (max <= min) {
        return min;
    }
    int64_t span = max - min;
    int64_t value = rand_int(span);
    return min + value;
}

// 随机布尔（true/false）。
int64_t rand_bool(void) {
    return rand_int(2) == 0 ? 0 : 1;
}

// UUID v4：xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx。
sw_string* random_uuid(void) {
    char buffer[37];
    for (int i = 0; i < 36; i++) {
        if (i == 8 || i == 13 || i == 18 || i == 23) {
            buffer[i] = '-';
            continue;
        }
        int v = (int)rand_int(16);
        if (i == 14) {
            v = 4;  // version 4
        }
        if (i == 19) {
            v = 8 + rand_int(4);  // variant 8/9/a/b
        }
        buffer[i] = v < 10 ? (char)('0' + v) : (char)('a' + v - 10);
    }
    buffer[36] = 0;
    return sw_string_from_literal(buffer, 36);
}

// 洗牌（原地交换，8 字节槽；string[]/int[] 均可）。
void sw_shuffle(sw_array* items) {
    if (items == NULL || items->len < 2) {
        return;
    }
    int64_t* data = (int64_t*)items->data;
    for (int64_t i = items->len - 1; i > 0; i--) {
        int64_t j = rand_int_range(0, i + 1);
        int64_t temp = data[i];
        data[i] = data[j];
        data[j] = temp;
    }
}

// 字节数格式化：1536 -> "1.5 KB"；支持 B/KB/MB/GB/TB。
sw_string* format_bytes(int64_t bytes) {
    char buffer[64];
    if (bytes < 0) {
        return sw_string_from_literal("-", 1);
    }
    static const char* units[] = {"B", "KB", "MB", "GB", "TB"};
    double value = (double)bytes;
    int unit = 0;
    while (value >= 1024.0 && unit < 4) {
        value /= 1024.0;
        unit++;
    }
    if (unit == 0) {
        snprintf(buffer, sizeof(buffer), "%lld B", (long long)bytes);
    } else if (value >= 100.0) {
        snprintf(buffer, sizeof(buffer), "%.0f %s", value, units[unit]);
    } else {
        // 最多两位小数，去掉尾零（1536 -> "1.5 KB"、1024 -> "1 KB"）。
        char num[32];
        snprintf(num, sizeof(num), "%.2f", value);
        int len = (int)strlen(num);
        while (len > 1 && num[len - 1] == '0') {
            len--;
        }
        if (len > 1 && num[len - 1] == '.') {
            len--;
        }
        num[len] = 0;
        snprintf(buffer, sizeof(buffer), "%s %s", num, units[unit]);
    }
    return sw_string_from_literal(buffer, (int64_t)strlen(buffer));
}

// 千分位格式化：1234567 -> "1,234,567"。
sw_string* format_thousands(int64_t value) {
    char buffer[64];
    if (value < 0) {
        char inner[64];
        snprintf(inner, sizeof(inner), "%lld", (long long)(-value));
        int len = (int)strlen(inner);
        int digits = len;
        int commas = (digits - 1) / 3;
        int out_len = 1 + digits + commas;
        char* out = (char*)sw_gc_alloc((uint64_t)out_len + 1);
        int w = 0;
        out[w++] = '-';
        for (int i = 0; i < digits; i++) {
            if (i > 0 && (digits - i) % 3 == 0) {
                out[w++] = ',';
            }
            out[w++] = inner[i];
        }
        out[w] = 0;
        return sw_string_from_literal(out, w);
    }
    snprintf(buffer, sizeof(buffer), "%lld", (long long)value);
    int len = (int)strlen(buffer);
    int commas = (len - 1) / 3;
    char* out = (char*)sw_gc_alloc((uint64_t)(len + commas + 1));
    int w = 0;
    for (int i = 0; i < len; i++) {
        if (i > 0 && (len - i) % 3 == 0) {
            out[w++] = ',';
        }
        out[w++] = buffer[i];
    }
    out[w] = 0;
    return sw_string_from_literal(out, w);
}

static const char sw_hex_digits[] = "0123456789abcdef";

// 整数转十六进制（小写，无前缀）。
sw_string* int_to_hex(int64_t value) {
    if (value == 0) {
        return sw_string_from_literal("0", 1);
    }
    int64_t v = value < 0 ? (int64_t)((uint64_t)value) : value;  // 按无符号位模式输出
    char buffer[32];
    int w = 0;
    int started = 0;
    for (int shift = 60; shift >= 0; shift -= 4) {
        int digit = (int)((v >> shift) & 0xF);
        if (digit != 0 || started) {
            buffer[w++] = sw_hex_digits[digit];
            started = 1;
        }
    }
    buffer[w] = 0;
    return sw_string_from_literal(buffer, w);
}

// 整数转八进制（无前缀）。
sw_string* int_to_oct(int64_t value) {
    if (value == 0) {
        return sw_string_from_literal("0", 1);
    }
    int64_t v = value < 0 ? (int64_t)((uint64_t)value) : value;
    char buffer[32];
    int w = 0;
    int started = 0;
    for (int shift = 60; shift >= 0; shift -= 3) {
        int digit = (int)((v >> shift) & 7);
        if (digit != 0 || started) {
            buffer[w++] = (char)('0' + digit);
            started = 1;
        }
    }
    buffer[w] = 0;
    return sw_string_from_literal(buffer, w);
}

// 整数转二进制（无前缀）。
sw_string* int_to_bin(int64_t value) {
    if (value == 0) {
        return sw_string_from_literal("0", 1);
    }
    int64_t v = value < 0 ? (int64_t)((uint64_t)value) : value;
    char buffer[80];
    int w = 0;
    int started = 0;
    for (int shift = 63; shift >= 0; shift--) {
        int digit = (int)((v >> shift) & 1);
        if (digit != 0 || started) {
            buffer[w++] = (char)('0' + digit);
            started = 1;
        }
    }
    buffer[w] = 0;
    return sw_string_from_literal(buffer, w);
}

// 按指定进制（2-36）解析字符串为整数；非法返回 0。
int64_t parse_int_radix(sw_string* text, int64_t radix) {
    if (text == NULL || radix < 2 || radix > 36) {
        return 0;
    }
    int64_t i = 0;
    int negative = 0;
    if (i < text->len && (text->data[i] == '-' || text->data[i] == '+')) {
        negative = text->data[i] == '-';
        i++;
    }
    int64_t result = 0;
    while (i < text->len) {
        char c = text->data[i];
        int digit;
        if (c >= '0' && c <= '9') {
            digit = c - '0';
        } else if (c >= 'a' && c <= 'z') {
            digit = c - 'a' + 10;
        } else if (c >= 'A' && c <= 'Z') {
            digit = c - 'A' + 10;
        } else {
            break;
        }
        if (digit >= radix) {
            break;
        }
        result = result * radix + digit;
        i++;
    }
    return negative ? -result : result;
}

// 驼峰转蛇形：helloWorld -> hello_world；HTTPServer -> http_server。
sw_string* to_snake_case(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t cap = text->len * 2 + 4;
    char* out = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t w = 0;
    for (int64_t i = 0; i < text->len; i++) {
        char c = text->data[i];
        if (c >= 'A' && c <= 'Z') {
            if (w > 0 && out[w - 1] != '_') {
                // 前一个是小写或数字才插下划线（避免连续大写中间插）。
                if ((text->data[i - 1] >= 'a' && text->data[i - 1] <= 'z') ||
                    (text->data[i - 1] >= '0' && text->data[i - 1] <= '9') ||
                    (i + 1 < text->len && text->data[i + 1] >= 'a' && text->data[i + 1] <= 'z' &&
                     text->data[i - 1] >= 'A' && text->data[i - 1] <= 'Z')) {
                    out[w++] = '_';
                }
            }
            out[w++] = (char)(c - 'A' + 'a');
        } else if (c == '-' || c == ' ') {
            if (w > 0 && out[w - 1] != '_') {
                out[w++] = '_';
            }
        } else {
            out[w++] = c;
        }
    }
    out[w] = 0;
    return sw_string_from_literal(out, w);
}

// 蛇形/空格/短横转驼峰：hello_world -> helloWorld；HelloWorld 首字母不变。
sw_string* to_camel_case(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t cap = text->len + 2;
    char* out = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t w = 0;
    int upper_next = 0;
    for (int64_t i = 0; i < text->len; i++) {
        char c = text->data[i];
        if (c == '_' || c == '-' || c == ' ') {
            upper_next = 1;
            continue;
        }
        if (upper_next && c >= 'a' && c <= 'z') {
            out[w++] = (char)(c - 'a' + 'A');
        } else {
            out[w++] = c;
        }
        upper_next = 0;
    }
    out[w] = 0;
    return sw_string_from_literal(out, w);
}

// 是否全部为字母（ASCII a-zA-Z，空串 false）。
int64_t is_alpha(sw_string* text) {
    if (text == NULL || text->len == 0) {
        return 0;
    }
    for (int64_t i = 0; i < text->len; i++) {
        char c = text->data[i];
        if (!((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z'))) {
            return 0;
        }
    }
    return 1;
}

// 是否全部为字母或数字（ASCII，空串 false）。
int64_t is_alnum(sw_string* text) {
    if (text == NULL || text->len == 0) {
        return 0;
    }
    for (int64_t i = 0; i < text->len; i++) {
        char c = text->data[i];
        if (!((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
              (c >= '0' && c <= '9'))) {
            return 0;
        }
    }
    return 1;
}

// 是否全部为标点（ASCII 可见非字母数字，空串 false）。
int64_t is_punct(sw_string* text) {
    if (text == NULL || text->len == 0) {
        return 0;
    }
    for (int64_t i = 0; i < text->len; i++) {
        unsigned char c = (unsigned char)text->data[i];
        if (!(c >= 33 && c <= 126) ||
            ((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9'))) {
            return 0;
        }
    }
    return 1;
}

// CSV 行解析：支持引号包裹（含引号内逗号/换行/双引号转义）。
sw_array* csv_parse_line(sw_string* text) {
    sw_array* out = sw_array_new(8, 16);
    if (text == NULL) {
        out->len = 0;
        return out;
    }
    int64_t cap = 64;
    char* field = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t used = 0;
    int64_t slot = 0;
    int64_t i = 0;
    int in_quotes = 0;
    while (i <= text->len) {
        char c = i < text->len ? text->data[i] : 0;
        if (in_quotes) {
            if (c == '"') {
                if (i + 1 < text->len && text->data[i + 1] == '"') {
                    field[used++] = '"';
                    i += 2;
                    continue;
                }
                in_quotes = 0;
                i++;
                continue;
            }
            if (c == 0) {
                break;
            }
            field[used++] = c;
            i++;
            continue;
        }
        if (c == '"' && used == 0) {
            in_quotes = 1;
            i++;
            continue;
        }
        if (c == ',' || c == 0) {
            if (slot >= out->len) {
                sw_array* bigger = sw_array_new(8, out->len * 2 + 1);
                for (int64_t k = 0; k < slot; k++) {
                    ((int64_t*)bigger->data)[k] = ((int64_t*)out->data)[k];
                }
                out = bigger;
            }
            field[used] = 0;
            ((int64_t*)out->data)[slot++] = (int64_t)sw_string_from_literal(field, used);
            used = 0;
            i++;
            continue;
        }
        field[used++] = c;
        i++;
    }
    out->len = slot;
    out->cap = slot;
    return out;
}

// CSV 行连接：字段含逗号/引号/换行时自动加引号并转义。
sw_string* csv_join(sw_array* items) {
    int64_t* data = (int64_t*)items->data;
    int64_t cap = 64;
    char* out = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t used = 0;
    for (int64_t i = 0; i < items->len; i++) {
        sw_string* field = (sw_string*)data[i];
        int need_quote = 0;
        for (int64_t k = 0; k < field->len; k++) {
            char c = field->data[k];
            if (c == ',' || c == '"' || c == '\n' || c == '\r') {
                need_quote = 1;
                break;
            }
        }
        if (i > 0) {
            out[used++] = ',';
        }
        if (need_quote) {
            out[used++] = '"';
            for (int64_t k = 0; k < field->len; k++) {
                char c = field->data[k];
                if (c == '"') {
                    out[used++] = '"';
                }
                out[used++] = c;
            }
            out[used++] = '"';
        } else {
            for (int64_t k = 0; k < field->len; k++) {
                out[used++] = field->data[k];
            }
        }
    }
    out[used] = 0;
    return sw_string_from_literal(out, used);
}

// 本地日期时间含毫秒："YYYY-MM-DD HH:MM:SS.mmm"。
sw_string* datetime_string_ms(int64_t milliseconds) {
    int64_t sec = milliseconds / 1000;
    int64_t ms = milliseconds % 1000;
    if (ms < 0) {
        ms += 1000;
        sec -= 1;
    }
    sw_string* base = datetime_string(sec);
    char* out = (char*)sw_gc_alloc((uint64_t)base->len + 8);
    int64_t used = 0;
    for (int64_t i = 0; i < base->len; i++) {
        out[used++] = base->data[i];
    }
    used += snprintf(out + used, 8, ".%03lld", (long long)ms);
    out[used] = 0;
    return sw_string_from_literal(out, used);
}

// ISO 8601 周数：周一为一周开始，跨年按 ISO 规则（1 月 4 日所在周为第 1 周）。
int64_t week_of_year(int64_t seconds) {
    int64_t year = year_of(seconds);
    int64_t jan4 = sw_time_from_parts(year, 1, 4, 0, 0, 0);
    // jan4 的星期：weekday_of 0=周日。ISO 周一=0。
    int64_t jan4_wday = weekday_of(jan4);
    int64_t jan4_iso = jan4_wday == 0 ? 6 : jan4_wday - 1;
    // jan4 所在周的周一（时间戳直接减秒，避免 Windows 分支秒字段溢出）。
    int64_t week1_monday = jan4 - jan4_iso * 86400;
    int64_t diff = time_diff(week1_monday, seconds, 0);
    if (diff < 0) {
        // 属于上一年第 52/53 周：回退到上年 1 月 4 日递归。
        int64_t prev_year = year - 1;
        int64_t prev_jan4 = sw_time_from_parts(prev_year, 1, 4, 0, 0, 0);
        int64_t pw = weekday_of(prev_jan4);
        int64_t pw_iso = pw == 0 ? 6 : pw - 1;
        int64_t pweek1 = prev_jan4 - pw_iso * 86400;
        int64_t pdiff = time_diff(pweek1, seconds, 0);
        return pdiff / 604800 + 1;
    }
    return diff / 604800 + 1;
}

// 按行写文件：每行一个字符串，行尾补 \n。
int64_t write_lines(sw_string* path, sw_array* lines) {
    if (path == NULL || lines == NULL) {
        return -1;
    }
    sw_file_handle* file = fopen(path->data, "wb");
    if (file == NULL) {
        return -1;
    }
    int64_t* data = (int64_t*)lines->data;
    int64_t total = 0;
    for (int64_t i = 0; i < lines->len; i++) {
        sw_string* line = (sw_string*)data[i];
        if (line != NULL && line->len > 0) {
            total += (int64_t)fwrite(line->data, 1, (uint64_t)line->len, file);
        }
        total += (int64_t)fwrite("\n", 1, 1, file);
    }
    fclose(file);
    return total;
}

// 临时文件路径：<temp_dir>/<prefix><随机数>.tmp。
sw_string* temp_file_path(sw_string* prefix) {
    sw_string* dir = sw_temp_dir();
    char* out = (char*)sw_gc_alloc(1024);
    int64_t used = 0;
    for (int64_t i = 0; i < dir->len && used < 400; i++) {
        out[used++] = dir->data[i];
    }
    if (used > 0 && out[used - 1] != '/' && out[used - 1] != '\\') {
#if defined(_WIN32)
        out[used++] = '\\';
#else
        out[used++] = '/';
#endif
    }
    for (int64_t i = 0; prefix != NULL && i < prefix->len && used < 400; i++) {
        out[used++] = prefix->data[i];
    }
    used += snprintf(out + used, 200, "%lld.tmp", (long long)now_ms());
    out[used] = 0;
    return sw_string_from_literal(out, used);
}

// ---------------------------------------------------------------------------
// 第一批：INI 解析 / 随机字符串 / JSON 美化 / 数组实用
// ---------------------------------------------------------------------------

// 解析 INI 文本为 map：无节键直接存；[section] 下键存 "section.key"。
// 支持 # 与 ; 注释、key = value、空行。
void* ini_parse(sw_string* text) {
    void* map = sw_map_new();
    if (text == NULL) {
        return map;
    }
    char* section = (char*)sw_gc_alloc(256);
    int section_len = 0;
    int64_t i = 0;
    while (i <= text->len) {
        // 取一行
        int64_t line_end = i;
        while (line_end < text->len && text->data[line_end] != '\n') {
            line_end++;
        }
        // 去首尾空白
        int64_t start = i;
        int64_t end = line_end;
        while (start < end && (text->data[start] == ' ' || text->data[start] == '\t' ||
                               text->data[start] == '\r')) {
            start++;
        }
        while (end > start && (text->data[end - 1] == ' ' || text->data[end - 1] == '\t' ||
                               text->data[end - 1] == '\r')) {
            end--;
        }
        if (end > start) {
            char first = text->data[start];
            if (first == '#' || first == ';') {
                // 注释
            } else if (first == '[') {
                int64_t close = start + 1;
                while (close < end && text->data[close] != ']') {
                    close++;
                }
                section_len = 0;
                for (int64_t k = start + 1; k < close && k < end && section_len < 255; k++) {
                    if (text->data[k] != ' ' && text->data[k] != '\t') {
                        section[section_len++] = text->data[k];
                    }
                }
                section[section_len] = 0;
            } else {
                // key = value
                int64_t eq = start;
                while (eq < end && text->data[eq] != '=') {
                    eq++;
                }
                if (eq < end) {
                    int64_t key_start = start;
                    int64_t key_end = eq;
                    while (key_start < key_end &&
                           (text->data[key_start] == ' ' || text->data[key_start] == '\t')) {
                        key_start++;
                    }
                    while (key_end > key_start &&
                           (text->data[key_end - 1] == ' ' || text->data[key_end - 1] == '\t')) {
                        key_end--;
                    }
                    int64_t value_start = eq + 1;
                    int64_t value_end = end;
                    while (value_start < value_end &&
                           (text->data[value_start] == ' ' || text->data[value_start] == '\t')) {
                        value_start++;
                    }
                    while (value_end > value_start &&
                           (text->data[value_end - 1] == ' ' || text->data[value_end - 1] == '\t')) {
                        value_end--;
                    }
                    // 去掉行尾注释（值后 # / ;）
                    for (int64_t k = value_start; k < value_end; k++) {
                        if (text->data[k] == '#' || text->data[k] == ';') {
                            value_end = k;
                            break;
                        }
                    }
                    while (value_end > value_start &&
                           (text->data[value_end - 1] == ' ' || text->data[value_end - 1] == '\t')) {
                        value_end--;
                    }
                    // 组装键名
                    int64_t key_cap = section_len + (key_end - key_start) + 2;
                    char* key = (char*)sw_gc_alloc((uint64_t)key_cap);
                    int64_t key_used = 0;
                    for (int64_t k = 0; k < section_len && key_used < key_cap - 1; k++) {
                        key[key_used++] = section[k];
                    }
                    if (section_len > 0) {
                        key[key_used++] = '.';
                    }
                    for (int64_t k = key_start; k < key_end && key_used < key_cap - 1; k++) {
                        key[key_used++] = text->data[k];
                    }
                    key[key_used] = 0;
                    sw_string* key_str = sw_string_from_literal(key, key_used);
                    sw_string* value_str =
                        sw_string_from_literal(text->data + value_start, value_end - value_start);
                    sw_map_set(map, key_str, value_str);
                }
            }
        }
        i = line_end + 1;
    }
    return map;
}

// 序列化 map 为 INI 文本（无节键在前，[section] 分组输出）。
sw_string* ini_save(void* map) {
    sw_array* keys = sw_map_keys(map);
    sw_array* values = sw_map_values(map);
    int64_t* kdata = (int64_t*)keys->data;
    int64_t* vdata = (int64_t*)values->data;
    int64_t cap = 256;
    char* buffer = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t used = 0;
    char* current_section = (char*)sw_gc_alloc(256);
    int current_len = 0;
    for (int64_t i = 0; i < keys->len; i++) {
        sw_string* key = (sw_string*)kdata[i];
        sw_string* value = (sw_string*)vdata[i];
        // 拆 section.key
        int64_t dot = -1;
        for (int64_t k = 0; k < key->len; k++) {
            if (key->data[k] == '.') {
                dot = k;
                break;
            }
        }
        if (dot >= 0) {
            char* sec = (char*)sw_gc_alloc((uint64_t)dot + 1);
            for (int64_t k = 0; k < dot; k++) {
                sec[k] = key->data[k];
            }
            sec[dot] = 0;
            if (current_len != dot ||
                memcmp(current_section, sec, (uint64_t)dot) != 0) {
                if (used + dot + 8 < cap) {
                    buffer[used++] = '[';
                    for (int64_t k = 0; k < dot; k++) {
                        buffer[used++] = sec[k];
                    }
                    buffer[used++] = ']';
                    buffer[used++] = '\n';
                }
                current_len = dot;
                for (int64_t k = 0; k < dot; k++) {
                    current_section[k] = sec[k];
                }
                current_section[dot] = 0;
            }
            if (used + key->len + value->len + 4 < cap) {
                for (int64_t k = dot + 1; k < key->len; k++) {
                    buffer[used++] = key->data[k];
                }
                buffer[used++] = '=';
                for (int64_t k = 0; k < value->len; k++) {
                    buffer[used++] = value->data[k];
                }
                buffer[used++] = '\n';
            }
        } else {
            if (used + key->len + value->len + 4 < cap) {
                for (int64_t k = 0; k < key->len; k++) {
                    buffer[used++] = key->data[k];
                }
                buffer[used++] = '=';
                for (int64_t k = 0; k < value->len; k++) {
                    buffer[used++] = value->data[k];
                }
                buffer[used++] = '\n';
            }
        }
    }
    buffer[used] = 0;
    return sw_string_from_literal(buffer, used);
}

// 随机字母数字字符串（A-Z a-z 0-9）。
sw_string* random_string(int64_t length) {
    if (length < 0) {
        length = 0;
    }
    static const char charset[] =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    char* out = (char*)sw_gc_alloc((uint64_t)length + 1);
    for (int64_t i = 0; i < length; i++) {
        out[i] = charset[rand_int_range(0, 62)];
    }
    out[length] = 0;
    return sw_string_from_literal(out, length);
}

// 随机十六进制 token（2*length 字符）。
sw_string* random_token(int64_t length) {
    if (length < 0) {
        length = 0;
    }
    static const char hex[] = "0123456789abcdef";
    char* out = (char*)sw_gc_alloc((uint64_t)(length * 2) + 1);
    for (int64_t i = 0; i < length; i++) {
        out[i * 2] = hex[rand_int_range(0, 16)];
        out[i * 2 + 1] = hex[rand_int_range(0, 16)];
    }
    out[length * 2] = 0;
    return sw_string_from_literal(out, length * 2);
}

// JSON 美化输出（缩进 2 空格）。
static void sw_json_write_pretty(
    sw_str_builder* builder,
    sw_json* value,
    int64_t depth
) {
    switch (value->kind) {
        case 0:
            sw_builder_append(builder, "null", 4);
            break;
        case 1:
            sw_builder_append(builder, value->int_value ? "true" : "false", value->int_value ? 4 : 5);
            break;
        case 2: {
            char number[32];
            int len = snprintf(number, sizeof(number), "%lld", (long long)value->int_value);
            sw_builder_append(builder, number, len);
            break;
        }
        case 3: {
            char number[64];
            int len = snprintf(number, sizeof(number), "%.17g", value->float_value);
            sw_builder_append(builder, number, len);
            break;
        }
        case 4:
            sw_json_escape_append(builder, value->string_value, value->length);
            break;
        case 5:
            if (value->length == 0) {
                sw_builder_append(builder, "[]", 2);
                break;
            }
            sw_builder_char(builder, '[');
            for (int64_t i = 0; i < value->length; i++) {
                if (i > 0) {
                    sw_builder_char(builder, ',');
                }
                sw_builder_char(builder, '\n');
                for (int64_t d = 0; d <= depth + 1; d++) {
                    sw_builder_append(builder, "  ", 2);
                }
                sw_json_write_pretty(builder, value->items[i], depth + 1);
            }
            sw_builder_char(builder, '\n');
            for (int64_t d = 0; d < depth; d++) {
                sw_builder_append(builder, "  ", 2);
            }
            sw_builder_char(builder, ']');
            break;
        case 6:
            if (value->length == 0) {
                sw_builder_append(builder, "{}", 2);
                break;
            }
            sw_builder_char(builder, '{');
            for (int64_t i = 0; i < value->length; i++) {
                if (i > 0) {
                    sw_builder_char(builder, ',');
                }
                sw_builder_char(builder, '\n');
                for (int64_t d = 0; d <= depth + 1; d++) {
                    sw_builder_append(builder, "  ", 2);
                }
                sw_json_escape_append(builder, value->keys[i], (int64_t)strlen(value->keys[i]));
                sw_builder_append(builder, ": ", 2);
                sw_json_write_pretty(builder, value->items[i], depth + 1);
            }
            sw_builder_char(builder, '\n');
            for (int64_t d = 0; d < depth; d++) {
                sw_builder_append(builder, "  ", 2);
            }
            sw_builder_char(builder, '}');
            break;
        default:
            sw_builder_append(builder, "null", 4);
    }
}

sw_string* json_stringify_pretty(void* value) {
    sw_json* node = (sw_json*)value;
    if (node == NULL) {
        return sw_string_from_literal("null", 4);
    }
    sw_str_builder builder = {NULL, 0, 0};
    sw_json_write_pretty(&builder, node, 0);
    sw_builder_grow(&builder, 1);
    builder.data[builder.len] = 0;
    sw_string* result = sw_string_from_literal(builder.data, builder.len);
    free(builder.data);
    return result;
}

// 生成整数序列 [start, end)：step 可为负；step=0 返回空数组。
sw_array* arr_range(int64_t start, int64_t end, int64_t step) {
    if (step == 0) {
        return sw_array_new(8, 0);
    }
    int64_t count = 0;
    if (step > 0) {
        if (end > start) {
            count = (end - start + step - 1) / step;
        }
    } else {
        if (end < start) {
            count = (start - end + (-step) - 1) / (-step);
        }
    }
    if (count < 0) {
        count = 0;
    }
    sw_array* array = sw_array_new(8, count);
    int64_t* data = (int64_t*)array->data;
    int64_t value = start;
    for (int64_t i = 0; i < count; i++) {
        data[i] = value;
        value += step;
    }
    return array;
}

// 生成 count 个 value 的 int[]。
sw_array* arr_fill(int64_t value, int64_t count) {
    if (count < 0) {
        count = 0;
    }
    sw_array* array = sw_array_new(8, count);
    int64_t* data = (int64_t*)array->data;
    for (int64_t i = 0; i < count; i++) {
        data[i] = value;
    }
    return array;
}

// 统计 int[] 中等于 value 的元素个数。
int64_t arr_count_int(sw_array* items, int64_t value) {
    if (items == NULL) {
        return 0;
    }
    int64_t* data = (int64_t*)items->data;
    int64_t count = 0;
    for (int64_t i = 0; i < items->len; i++) {
        if (data[i] == value) {
            count++;
        }
    }
    return count;
}

// int[] 平均值；空数组返回 0.0。
double arr_avg_int(sw_array* items) {
    if (items == NULL || items->len == 0) {
        return 0.0;
    }
    int64_t* data = (int64_t*)items->data;
    double total = 0.0;
    for (int64_t i = 0; i < items->len; i++) {
        total += (double)data[i];
    }
    return total / (double)items->len;
}

// ---------------------------------------------------------------------------
// 第二批：UTC 时间字段 / 日历加减 / which / mkdtemp / UDP / regex 捕获
// ---------------------------------------------------------------------------

// UTC 字段提取（gmtime 语义）：field 0=年 1=月 2=星期 3=日 4=时 5=分 6=秒。
static int sw_utc_field(int64_t seconds, int field) {
#if defined(_WIN32)
    extern int FileTimeToSystemTime(const void* file_time, void* system_time);
    uint64_t since_1601 = ((uint64_t)seconds + 11644473600ULL) * 10000000ULL;
    unsigned char ft[8];
    unsigned char st[16];
    *(unsigned int*)ft = (unsigned int)since_1601;
    *(unsigned int*)(ft + 4) = (unsigned int)(since_1601 >> 32);
    if (!FileTimeToSystemTime(ft, st)) {
        return -1;
    }
    static const int offsets[] = {0, 2, 4, 6, 8, 10, 12};
    return *(unsigned short*)(st + offsets[field]);
#else
    extern void* gmtime_r(const void* time, void* tm);
    unsigned char tm[64];
    unsigned char t[8];
    *(int64_t*)t = seconds;
    if (gmtime_r(t, tm) == NULL) {
        return -1;
    }
    static const int offsets[] = {20, 16, 24, 12, 8, 4, 0};
    int value = *(int*)(tm + offsets[field]);
    if (field == 0) {
        return value + 1900;
    }
    if (field == 1) {
        return value + 1;
    }
    return value;
#endif
}

int64_t utc_year_of(int64_t seconds) {
    return sw_utc_field(seconds, 0);
}

int64_t utc_month_of(int64_t seconds) {
    return sw_utc_field(seconds, 1);
}

int64_t utc_day_of(int64_t seconds) {
    return sw_utc_field(seconds, 3);
}

int64_t utc_hour_of(int64_t seconds) {
    return sw_utc_field(seconds, 4);
}

int64_t utc_minute_of(int64_t seconds) {
    return sw_utc_field(seconds, 5);
}

int64_t utc_second_of(int64_t seconds) {
    return sw_utc_field(seconds, 6);
}

int64_t utc_weekday_of(int64_t seconds) {
    return sw_utc_field(seconds, 2);
}

// 把本地时间戳的日历字段加 months/years（月末自动收敛，如 1-31 +1月 → 2-28）。
static int64_t sw_calendar_add(int64_t seconds, int64_t months) {
    int year = sw_time_field(seconds, 0);
    int month = sw_time_field(seconds, 1);
    int day = sw_time_field(seconds, 3);
    int hour = sw_time_field(seconds, 4);
    int minute = sw_time_field(seconds, 5);
    int second = sw_time_field(seconds, 6);
    if (year < 0 || month < 0) {
        return -1;
    }
    int64_t total_months = (int64_t)year * 12 + (month - 1) + months;
    int new_year = (int)(total_months / 12);
    int new_month = (int)(total_months % 12) + 1;
    if (new_month <= 0) {
        new_month += 12;
        new_year -= 1;
    }
    int max_day = days_in_month(new_year, new_month);
    if (day > max_day) {
        day = max_day;
    }
    return sw_time_from_parts(new_year, new_month, day, hour, minute, second);
}

int64_t add_months(int64_t seconds, int64_t months) {
    return sw_calendar_add(seconds, months);
}

int64_t add_years(int64_t seconds, int64_t years) {
    return sw_calendar_add(seconds, years * 12);
}

// 按 PATH 查找可执行文件完整路径；未找到返回空串。
sw_string* os_which(sw_string* name) {
    if (name == NULL || name->len == 0) {
        return sw_string_from_literal("", 0);
    }
    sw_string* path_env = sw_getenv(sw_string_from_literal("PATH", 4));
    if (path_env == NULL) {
        return sw_string_from_literal("", 0);
    }
    char* name_c = (char*)sw_gc_alloc((uint64_t)name->len + 1);
    memcpy(name_c, name->data, (uint64_t)name->len);
    name_c[name->len] = 0;
#if defined(_WIN32)
    const char sep = ';';
#else
    const char sep = ':';
#endif
    int64_t i = 0;
    while (i <= path_env->len) {
        int64_t seg_end = i;
        while (seg_end < path_env->len && path_env->data[seg_end] != sep) {
            seg_end++;
        }
        int64_t seg_len = seg_end - i;
        char* dir = (char*)sw_gc_alloc((uint64_t)seg_len + 2);
        for (int64_t k = 0; k < seg_len; k++) {
            dir[k] = path_env->data[i + k];
        }
        dir[seg_len] = 0;
        char* candidate = (char*)sw_gc_alloc((uint64_t)seg_len + (uint64_t)name->len + 4);
        int64_t used = 0;
        for (int64_t k = 0; dir[k]; k++) {
            candidate[used++] = dir[k];
        }
        if (used > 0 && candidate[used - 1] != '/' && candidate[used - 1] != '\\') {
            candidate[used++] = '/';
        }
        for (int64_t k = 0; k < name->len; k++) {
            candidate[used++] = name->data[k];
        }
        candidate[used] = 0;
#if defined(_WIN32)
        // Windows 还要尝试常见可执行扩展名。
        const char* exts[] = {"", ".exe", ".bat", ".cmd", ".com"};
        for (int e = 0; e < 5; e++) {
            int ex = (int)strlen(exts[e]);
            char* full = (char*)sw_gc_alloc((uint64_t)used + (uint64_t)ex + 1);
            for (int64_t k = 0; k < used; k++) {
                full[k] = candidate[k];
            }
            for (int k = 0; k < ex; k++) {
                full[used + k] = exts[e][k];
            }
            full[used + ex] = 0;
            sw_file_handle* f = fopen(full, "rb");
            if (f != NULL) {
                fclose(f);
                return sw_string_from_literal(full, used + ex);
            }
        }
#else
        sw_file_handle* f = fopen(candidate, "rb");
        if (f != NULL) {
            fclose(f);
            return sw_string_from_literal(candidate, used);
        }
#endif
        i = seg_end + 1;
    }
    return sw_string_from_literal("", 0);
}

// 创建唯一临时目录，返回完整路径；失败返回空串。
sw_string* mkdtemp(sw_string* prefix) {
    for (int attempt = 0; attempt < 10; attempt++) {
        sw_string* base = temp_file_path(prefix);
        int64_t cap = base->len + 4;
        char* path = (char*)sw_gc_alloc((uint64_t)cap);
        int64_t used = 0;
        for (int64_t i = 0; i < base->len; i++) {
            path[used++] = base->data[i];
        }
        // 去掉 .tmp 后缀，换成随机目录名
        if (used >= 4 && path[used - 4] == '.' && path[used - 3] == 't' &&
            path[used - 2] == 'm' && path[used - 1] == 'p') {
            used -= 4;
        }
        used += snprintf(path + used, 16, "-%04lld", (long long)rand_int_range(0, 10000));
        path[used] = 0;
        if (sw_mkdir(sw_string_from_literal(path, used)) == 0) {
            return sw_string_from_literal(path, used);
        }
    }
    return sw_string_from_literal("", 0);
}

// UDP：创建数据报 socket（SOCK_DGRAM）。
int64_t udp_socket(void) {
#if defined(_WIN32)
    extern int WSAStartup(unsigned short version, void* data);
    static int started = 0;
    if (!started) {
        unsigned char data[408];
        memset(data, 0, sizeof(data));
        if (WSAStartup(0x0202, data) == 0) {
            started = 1;
        }
    }
    extern uintptr_t socket(int domain, int type, int protocol);
    uintptr_t sock = socket(2, 2, 17);  // AF_INET / SOCK_DGRAM / UDP
    return sock == (uintptr_t)~0 ? -1 : (int64_t)sock;
#else
    extern int socket(int domain, int type, int protocol);
    int sock = socket(2, 2, 17);
    return sock < 0 ? -1 : sock;
#endif
}

// UDP 绑定本地端口（0 由系统分配）。
int64_t udp_bind(int64_t fd, int64_t port) {
    if (fd < 0 || port < 0 || port > 65535) {
        return -1;
    }
    unsigned char addr[16];
    memset(addr, 0, sizeof(addr));
    *(unsigned short*)(addr + 0) = 2;  // AF_INET
    *(unsigned short*)(addr + 2) = (unsigned short)sw_net_be16(port);
#if defined(_WIN32)
    extern int bind(uintptr_t s, const void* name, int namelen);
    return bind((uintptr_t)fd, addr, 16) == 0 ? 0 : -1;
#else
    extern int bind(int s, const void* name, unsigned int namelen);
    return bind((int)fd, addr, 16) == 0 ? 0 : -1;
#endif
}

// UDP 发送数据报到 host:port；返回发送字节数，失败返回 -1。
int64_t udp_send(int64_t fd, sw_string* host, int64_t port, sw_string* data) {
    if (fd < 0 || host == NULL || data == NULL || port < 0 || port > 65535) {
        return -1;
    }
    char* host_copy = (char*)sw_gc_alloc((uint64_t)host->len + 1);
    memcpy(host_copy, host->data, (uint64_t)host->len);
    host_copy[host->len] = 0;
    char port_text[16];
    snprintf(port_text, sizeof(port_text), "%lld", (long long)port);
#if defined(_WIN32)
    extern int getaddrinfo(const char* node, const char* service, const void* hints, void** result);
    extern void freeaddrinfo(void* result);
    extern int sendto(uintptr_t s, const char* buf, int len, int flags, const void* to, int tolen);
#else
    extern int getaddrinfo(const char* node, const char* service, const void* hints, void** result);
    extern void freeaddrinfo(void* result);
    extern long sendto(int s, const void* buf, uintptr_t len, int flags, const void* to, unsigned int tolen);
#endif
    sw_addrinfo hints;
    memset(&hints, 0, sizeof(hints));
    hints.family = 2;
    hints.socktype = 2;  // SOCK_DGRAM
    void* results = NULL;
    if (getaddrinfo(host_copy, port_text, &hints, &results) != 0 || results == NULL) {
        return -1;
    }
    sw_addrinfo* info = (sw_addrinfo*)results;
    int64_t max = data->len > 65507 ? 65507 : data->len;
#if defined(_WIN32)
    int sent = sendto((uintptr_t)fd, data->data, (int)max, 0, info->addr, (int)info->addrlen);
    int result = sent < 0 ? -1 : (int64_t)sent;
#else
    long sent = sendto((int)fd, data->data, (uintptr_t)max, 0, info->addr, (unsigned int)info->addrlen);
    int64_t result = sent < 0 ? -1 : (int64_t)sent;
#endif
    freeaddrinfo(results);
    return result;
}

// UDP 接收数据报；返回收到的文本，失败/超时返回空串。
sw_string* udp_recv(int64_t fd, int64_t max_bytes) {
    if (fd < 0 || max_bytes <= 0) {
        return sw_string_from_literal("", 0);
    }
    if (max_bytes > 65507) {
        max_bytes = 65507;
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)max_bytes + 1);
#if defined(_WIN32)
    extern int recvfrom(uintptr_t s, char* buf, int len, int flags, void* from, int* fromlen);
    int got = recvfrom((uintptr_t)fd, buffer, (int)max_bytes, 0, NULL, NULL);
#else
    extern long recvfrom(int s, void* buf, uintptr_t len, int flags, void* from, unsigned int* fromlen);
    long got = recvfrom((int)fd, buffer, (uintptr_t)max_bytes, 0, NULL, NULL);
#endif
    if (got <= 0) {
        return sw_string_from_literal("", 0);
    }
    buffer[got] = 0;
    return sw_string_from_literal(buffer, (int64_t)got);
}

int64_t udp_close(int64_t fd) {
#if defined(_WIN32)
    extern int closesocket(uintptr_t s);
    return closesocket((uintptr_t)fd) == 0 ? 0 : -1;
#else
    extern int close(int s);
    return close((int)fd) == 0 ? 0 : -1;
#endif
}

// ---------------------------------------------------------------------------
// 基础库：JSON 构建 / 模板渲染 / 表格格式化
// ---------------------------------------------------------------------------

// 创建空 JSON 对象（可继续 json_object_set）。
void* json_object_new(void) {
    return sw_json_make(6);
}

// 创建空 JSON 数组（可继续 json_array_append）。
void* json_array_new(void) {
    return sw_json_make(5);
}

// 创建 JSON 字符串节点。
void* json_string_new(sw_string* text) {
    sw_json* value = sw_json_make(4);
    if (text == NULL) {
        value->string_value = (char*)sw_gc_alloc(1);
        value->string_value[0] = 0;
        value->length = 0;
        return value;
    }
    value->string_value = (char*)sw_gc_alloc((uint64_t)text->len + 1);
    memcpy(value->string_value, text->data, (uint64_t)text->len);
    value->string_value[text->len] = 0;
    value->length = text->len;
    return value;
}

// 创建 JSON 整数节点。
void* json_int_new(int64_t value) {
    sw_json* node = sw_json_make(2);
    node->int_value = value;
    return node;
}

// 创建 JSON 浮点节点。
void* json_float_new(double value) {
    sw_json* node = sw_json_make(3);
    node->float_value = value;
    return node;
}

// 创建 JSON 布尔节点。
void* json_bool_new(int64_t value) {
    sw_json* node = sw_json_make(1);
    node->int_value = value ? 1 : 0;
    return node;
}

// 创建 JSON null 节点。
void* json_null_new(void) {
    return sw_json_make(0);
}

// 给 JSON 对象设置键值（覆盖同名键）；成功返回 0，失败返回 -1。
int64_t json_object_set(void* object, sw_string* key, void* value) {
    sw_json* obj = (sw_json*)object;
    if (obj == NULL || obj->kind != 6 || key == NULL) {
        return -1;
    }
    // 同名键覆盖
    for (int64_t i = 0; i < obj->length; i++) {
        int64_t key_len = (int64_t)strlen(obj->keys[i]);
        if (key_len == key->len && memcmp(obj->keys[i], key->data, (uint64_t)key->len) == 0) {
            obj->items[i] = (sw_json*)value;
            return 0;
        }
    }
    int64_t new_len = obj->length + 1;
    sw_json** new_items = (sw_json**)sw_gc_alloc((uint64_t)new_len * sizeof(sw_json*));
    char** new_keys = (char**)sw_gc_alloc((uint64_t)new_len * sizeof(char*));
    for (int64_t i = 0; i < obj->length; i++) {
        new_items[i] = obj->items[i];
        new_keys[i] = obj->keys[i];
    }
    char* key_copy = (char*)sw_gc_alloc((uint64_t)key->len + 1);
    memcpy(key_copy, key->data, (uint64_t)key->len);
    key_copy[key->len] = 0;
    new_keys[obj->length] = key_copy;
    new_items[obj->length] = (sw_json*)value;
    obj->items = new_items;
    obj->keys = new_keys;
    obj->length = new_len;
    return 0;
}

// 给 JSON 数组追加元素；成功返回 0，失败返回 -1。
int64_t json_array_append(void* array, void* value) {
    sw_json* arr = (sw_json*)array;
    if (arr == NULL || arr->kind != 5) {
        return -1;
    }
    int64_t new_len = arr->length + 1;
    sw_json** new_items = (sw_json**)sw_gc_alloc((uint64_t)new_len * sizeof(sw_json*));
    for (int64_t i = 0; i < arr->length; i++) {
        new_items[i] = arr->items[i];
    }
    new_items[arr->length] = (sw_json*)value;
    arr->items = new_items;
    arr->length = new_len;
    return 0;
}

// 模板渲染：把 text 中的 {key} 占位符替换为 map 中对应值；
// {{ 转义为字面 {；未知键替换为空串。
sw_string* render_template(sw_string* text, void* map) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t cap = text->len * 2 + 64;
    char* buffer = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t used = 0;
    int64_t i = 0;
    while (i < text->len) {
        if (text->data[i] == '{' && i + 1 < text->len && text->data[i + 1] == '{') {
            if (used + 1 < cap) {
                buffer[used++] = '{';
            }
            i += 2;
            continue;
        }
        if (text->data[i] == '}' && i + 1 < text->len && text->data[i + 1] == '}') {
            if (used + 1 < cap) {
                buffer[used++] = '}';
            }
            i += 2;
            continue;
        }
        if (text->data[i] == '{') {
            int64_t close = i + 1;
            while (close < text->len && text->data[close] != '}') {
                close++;
            }
            if (close < text->len) {
                sw_string* key =
                    sw_string_from_literal(text->data + i + 1, close - i - 1);
                sw_string* value = sw_map_get(map, key);
                if (value != NULL) {
                    for (int64_t k = 0; k < value->len && used + 1 < cap; k++) {
                        buffer[used++] = value->data[k];
                    }
                }
                i = close + 1;
                continue;
            }
        }
        if (used + 1 < cap) {
            buffer[used++] = text->data[i];
        }
        i++;
    }
    buffer[used] = 0;
    return sw_string_from_literal(buffer, used);
}

// 表格格式化：headers 为 string[]，rows 为 string[][]（每行元素个数与列数
// 不必一致，缺失按空串补）。列按内容最大宽度左对齐，用空格分隔。
sw_string* format_table(sw_array* headers, sw_array* rows) {
    int64_t* hdata = headers != NULL ? (int64_t*)headers->data : NULL;
    int64_t col_count = headers != NULL ? headers->len : 0;
    // 计算列数（取所有行最大宽度）
    int64_t* row_data = rows != NULL ? (int64_t*)rows->data : NULL;
    for (int64_t r = 0; rows != NULL && r < rows->len; r++) {
        sw_array* row = (sw_array*)row_data[r];
        if (row != NULL && row->len > col_count) {
            col_count = row->len;
        }
    }
    if (col_count <= 0) {
        return sw_string_from_literal("", 0);
    }
    // 每列最大宽度
    int64_t* widths = (int64_t*)sw_gc_alloc((uint64_t)col_count * 8);
    for (int64_t c = 0; c < col_count; c++) {
        widths[c] = 0;
    }
    for (int64_t c = 0; c < col_count && headers != NULL && c < headers->len; c++) {
        sw_string* cell = (sw_string*)hdata[c];
        if (cell != NULL && cell->len > widths[c]) {
            widths[c] = cell->len;
        }
    }
    for (int64_t r = 0; rows != NULL && r < rows->len; r++) {
        sw_array* row = (sw_array*)row_data[r];
        int64_t* cdata = row != NULL ? (int64_t*)row->data : NULL;
        for (int64_t c = 0; row != NULL && c < row->len && c < col_count; c++) {
            sw_string* cell = (sw_string*)cdata[c];
            if (cell != NULL && cell->len > widths[c]) {
                widths[c] = cell->len;
            }
        }
    }
    // 估算容量并逐行输出
    int64_t cap = 64;
    for (int64_t c = 0; c < col_count; c++) {
        cap += widths[c] + 2;
    }
    cap *= (headers != NULL ? headers->len : 0) + (rows != NULL ? rows->len : 0) + 2;
    if (cap < 256) {
        cap = 256;
    }
    char* buffer = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t used = 0;
    // 表头
    if (headers != NULL && headers->len > 0) {
        for (int64_t c = 0; c < col_count; c++) {
            sw_string* cell = c < headers->len ? (sw_string*)hdata[c] : NULL;
            int64_t len = cell != NULL ? cell->len : 0;
            for (int64_t k = 0; k < len && used + 1 < cap; k++) {
                buffer[used++] = cell->data[k];
            }
            for (int64_t k = len; k < widths[c] && used + 1 < cap; k++) {
                buffer[used++] = ' ';
            }
            if (c + 1 < col_count && used + 1 < cap) {
                buffer[used++] = ' ';
            }
        }
        buffer[used++] = '\n';
        // 分隔线
        for (int64_t c = 0; c < col_count; c++) {
            for (int64_t k = 0; k < widths[c] && used + 1 < cap; k++) {
                buffer[used++] = '-';
            }
            if (c + 1 < col_count && used + 1 < cap) {
                buffer[used++] = ' ';
            }
        }
        buffer[used++] = '\n';
    }
    // 数据行
    for (int64_t r = 0; rows != NULL && r < rows->len; r++) {
        sw_array* row = (sw_array*)row_data[r];
        int64_t* cdata = row != NULL ? (int64_t*)row->data : NULL;
        for (int64_t c = 0; c < col_count; c++) {
            sw_string* cell =
                (row != NULL && c < row->len) ? (sw_string*)cdata[c] : NULL;
            int64_t len = cell != NULL ? cell->len : 0;
            for (int64_t k = 0; k < len && used + 1 < cap; k++) {
                buffer[used++] = cell->data[k];
            }
            for (int64_t k = len; k < widths[c] && used + 1 < cap; k++) {
                buffer[used++] = ' ';
            }
            if (c + 1 < col_count && used + 1 < cap) {
                buffer[used++] = ' ';
            }
        }
        if (used + 1 < cap) {
            buffer[used++] = '\n';
        }
    }
    buffer[used] = 0;
    return sw_string_from_literal(buffer, used);
}

// ---------------------------------------------------------------------------
// 第二批：数组补充（zip/最后位置/极值位置）/ TOML / slugify
// ---------------------------------------------------------------------------

// string[] 中 value 最后一次出现的位置；不存在返回 -1。
int64_t last_index_of_string(sw_array* items, sw_string* value) {
    if (items == NULL || value == NULL) {
        return -1;
    }
    sw_string** data = (sw_string**)items->data;
    for (int64_t i = items->len - 1; i >= 0; i--) {
        if (string_eq(data[i], value)) {
            return i;
        }
    }
    return -1;
}

// int[] 中 value 最后一次出现的位置；不存在返回 -1。
int64_t last_index_of_int(sw_array* items, int64_t value) {
    if (items == NULL) {
        return -1;
    }
    int64_t* data = (int64_t*)items->data;
    for (int64_t i = items->len - 1; i >= 0; i--) {
        if (data[i] == value) {
            return i;
        }
    }
    return -1;
}

// float[] 中 value 最后一次出现的位置；不存在返回 -1。
int64_t last_index_of_float(sw_array* items, double value) {
    if (items == NULL) {
        return -1;
    }
    double* data = (double*)items->data;
    for (int64_t i = items->len - 1; i >= 0; i--) {
        if (data[i] == value) {
            return i;
        }
    }
    return -1;
}

// int[] 最小值所在位置；空数组返回 -1。
int64_t min_index_int(sw_array* items) {
    if (items == NULL || items->len == 0) {
        return -1;
    }
    int64_t* data = (int64_t*)items->data;
    int64_t best = 0;
    for (int64_t i = 1; i < items->len; i++) {
        if (data[i] < data[best]) {
            best = i;
        }
    }
    return best;
}

// int[] 最大值所在位置；空数组返回 -1。
int64_t max_index_int(sw_array* items) {
    if (items == NULL || items->len == 0) {
        return -1;
    }
    int64_t* data = (int64_t*)items->data;
    int64_t best = 0;
    for (int64_t i = 1; i < items->len; i++) {
        if (data[i] > data[best]) {
            best = i;
        }
    }
    return best;
}

// 两个 string[] 按位置配对，返回 string[][]（每行两元素，短者为准）。
sw_array* zip_strings(sw_array* a, sw_array* b) {
    int64_t count = 0;
    if (a != NULL && b != NULL) {
        count = a->len < b->len ? a->len : b->len;
    }
    sw_array* out = sw_array_new(8, count);
    int64_t* odata = (int64_t*)out->data;
    int64_t* adata = a != NULL ? (int64_t*)a->data : NULL;
    int64_t* bdata = b != NULL ? (int64_t*)b->data : NULL;
    for (int64_t i = 0; i < count; i++) {
        sw_array* row = sw_array_new(8, 2);
        ((int64_t*)row->data)[0] = adata[i];
        ((int64_t*)row->data)[1] = bdata[i];
        odata[i] = (int64_t)row;
    }
    return out;
}

// 解析 TOML 文本为 map：支持 [section]、key=value（字符串去引号、
// 数字/布尔/数组按文本存）、# 注释、空行。
void* toml_parse(sw_string* text) {
    void* map = sw_map_new();
    if (text == NULL) {
        return map;
    }
    char* section = (char*)sw_gc_alloc(256);
    int section_len = 0;
    int64_t i = 0;
    while (i <= text->len) {
        int64_t line_end = i;
        while (line_end < text->len && text->data[line_end] != '\n') {
            line_end++;
        }
        int64_t start = i;
        int64_t end = line_end;
        while (start < end && (text->data[start] == ' ' || text->data[start] == '\t' ||
                               text->data[start] == '\r')) {
            start++;
        }
        while (end > start && (text->data[end - 1] == ' ' || text->data[end - 1] == '\t' ||
                               text->data[end - 1] == '\r')) {
            end--;
        }
        if (end > start) {
            char first = text->data[start];
            if (first == '#') {
                // 注释
            } else if (first == '[') {
                int64_t close = start + 1;
                while (close < end && text->data[close] != ']') {
                    close++;
                }
                section_len = 0;
                for (int64_t k = start + 1; k < close && k < end && section_len < 255; k++) {
                    if (text->data[k] != ' ' && text->data[k] != '\t') {
                        section[section_len++] = text->data[k];
                    }
                }
                section[section_len] = 0;
            } else {
                int64_t eq = start;
                while (eq < end && text->data[eq] != '=') {
                    eq++;
                }
                if (eq < end) {
                    int64_t key_start = start;
                    int64_t key_end = eq;
                    while (key_start < key_end &&
                           (text->data[key_start] == ' ' || text->data[key_start] == '\t')) {
                        key_start++;
                    }
                    while (key_end > key_start &&
                           (text->data[key_end - 1] == ' ' || text->data[key_end - 1] == '\t')) {
                        key_end--;
                    }
                    int64_t value_start = eq + 1;
                    int64_t value_end = end;
                    while (value_start < value_end &&
                           (text->data[value_start] == ' ' || text->data[value_start] == '\t')) {
                        value_start++;
                    }
                    while (value_end > value_start &&
                           (text->data[value_end - 1] == ' ' || text->data[value_end - 1] == '\t')) {
                        value_end--;
                    }
                    // 去掉行尾 # 注释（字符串内忽略，简化处理）
                    for (int64_t k = value_start; k < value_end; k++) {
                        if (text->data[k] == '#') {
                            value_end = k;
                            break;
                        }
                    }
                    while (value_end > value_start &&
                           (text->data[value_end - 1] == ' ' || text->data[value_end - 1] == '\t')) {
                        value_end--;
                    }
                    // 字符串去引号
                    if (value_end - value_start >= 2 &&
                        text->data[value_start] == '"' &&
                        text->data[value_end - 1] == '"') {
                        value_start++;
                        value_end--;
                    }
                    int64_t key_cap = section_len + (key_end - key_start) + 2;
                    char* key = (char*)sw_gc_alloc((uint64_t)key_cap);
                    int64_t key_used = 0;
                    for (int64_t k = 0; k < section_len && key_used < key_cap - 1; k++) {
                        key[key_used++] = section[k];
                    }
                    if (section_len > 0) {
                        key[key_used++] = '.';
                    }
                    for (int64_t k = key_start; k < key_end && key_used < key_cap - 1; k++) {
                        key[key_used++] = text->data[k];
                    }
                    key[key_used] = 0;
                    sw_map_set(
                        map,
                        sw_string_from_literal(key, key_used),
                        sw_string_from_literal(text->data + value_start, value_end - value_start)
                    );
                }
            }
        }
        i = line_end + 1;
    }
    return map;
}

// 文本转 URL 友好 slug：小写，字母数字保留（中文保留），其余转 '-'，
// 压缩连续 '-' 并去首尾。
sw_string* slugify(sw_string* text) {
    if (text == NULL) {
        return sw_string_from_literal("", 0);
    }
    int64_t cap = text->len + 1;
    char* out = (char*)sw_gc_alloc((uint64_t)cap);
    int64_t w = 0;
    for (int64_t i = 0; i < text->len; i++) {
        unsigned char c = (unsigned char)text->data[i];
        if ((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
            (c >= '0' && c <= '9')) {
            out[w++] = (char)(c >= 'A' && c <= 'Z' ? c + 32 : c);
        } else if (c >= 0x80) {
            // 非 ASCII（中文等多字节）原样保留
            out[w++] = (char)c;
        } else if (w > 0 && out[w - 1] != '-') {
            out[w++] = '-';
        }
    }
    // 去尾 '-'
    while (w > 0 && out[w - 1] == '-') {
        w--;
    }
    out[w] = 0;
    return sw_string_from_literal(out, w);
}

// 莱文斯坦编辑距离（字节级 DP；模糊搜索/纠错提示用）。
int64_t edit_distance(sw_string* a, sw_string* b) {
    int64_t alen = a != NULL ? a->len : 0;
    int64_t blen = b != NULL ? b->len : 0;
    if (alen == 0) {
        return blen;
    }
    if (blen == 0) {
        return alen;
    }
    int64_t* prev = (int64_t*)malloc((uint64_t)(blen + 1) * sizeof(int64_t));
    int64_t* curr = (int64_t*)malloc((uint64_t)(blen + 1) * sizeof(int64_t));
    if (prev == NULL || curr == NULL) {
        if (prev != NULL) {
            free(prev);
        }
        if (curr != NULL) {
            free(curr);
        }
        return 0;
    }
    for (int64_t j = 0; j <= blen; j++) {
        prev[j] = j;
    }
    for (int64_t i = 1; i <= alen; i++) {
        curr[0] = i;
        for (int64_t j = 1; j <= blen; j++) {
            int64_t cost = a->data[i - 1] == b->data[j - 1] ? 0 : 1;
            int64_t del = prev[j] + 1;
            int64_t ins = curr[j - 1] + 1;
            int64_t sub = prev[j - 1] + cost;
            int64_t best = del < ins ? del : ins;
            curr[j] = best < sub ? best : sub;
        }
        int64_t* swap = prev;
        prev = curr;
        curr = swap;
    }
    int64_t result = prev[blen];
    free(prev);
    free(curr);
    return result;
}
