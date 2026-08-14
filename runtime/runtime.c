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
extern void* memcpy(void* dest, const void* src, sw_size count);
extern void* memset(void* dest, int value, sw_size count);
extern int memcmp(const void* a, const void* b, sw_size count);
extern int snprintf(char* buffer, sw_size size, const char* format, ...);
extern uint64_t fwrite(const void* data, sw_size size, sw_size count, void* stream);
extern int fputc(int character, void* stream);
extern uint64_t strlen(const char* text);
extern void exit(int code);
#if defined(_WIN32)
extern void* __acrt_iob_func(unsigned int index);
#define stdout __acrt_iob_func(1)
#define stdin __acrt_iob_func(0)
#elif defined(__APPLE__)
// macOS 上 stdin/stdout 是宏，真实符号为 ___stdinp / ___stdoutp。
extern void* __stdinp;
extern void* __stdoutp;
#define stdin __stdinp
#define stdout __stdoutp
#else
extern void* stdin;
extern void* stdout;
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
#define SW_GC_MAX_THRESHOLD (128u << 20)
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
    uintptr_t optional = image_base + e_lfanew + 24;
    uint32_t size_of_image = *(uint32_t*)(optional + 0x38);
    sw_gc_add_data_range(image_base, image_base + size_of_image);
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
    for (sw_gc_block* block = sw_gc_blocks; block != NULL; block = block->next) {
        uintptr_t payload = (uintptr_t)((char*)block + SW_GC_HEADER_SIZE);
        sw_gc_scan_words(payload, payload + block->size);
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
    if (!sw_gc_in_collect && sw_gc_allocated_since_collect >= sw_gc_threshold) {
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

void sw_print_string(sw_string* string) {
    if (string->len > 0) {
        fwrite(string->data, 1, (uint64_t)string->len, stdout);
    }
}

void println(sw_string* string) {
    sw_print_string(string);
    fputc('\n', stdout);
}

void print(sw_string* string) {
    sw_print_string(string);
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
    sw_file_handle* file = fopen(path->data, "rb");
    if (file == NULL) {
        return 0;
    }
    fclose(file);
    return 1;
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
    return DeleteFileA(path->data) ? 0 : -1;
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

// 可变参数运行时类型标签（与编译器 SW_TAG_* 保持一致）。
#define SW_TAG_INT 0
#define SW_TAG_FLOAT 1
#define SW_TAG_STR 2
#define SW_TAG_BOOL 3
#define SW_TAG_CHAR 4

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
    sw_frame* frame = (sw_frame*)malloc(sizeof(sw_frame));
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
    free(current);
}

void sw_throw(void* value, int64_t type_id) {
    sw_frame* frame = sw_current_frame;
    sw_exception* exception = (sw_exception*)malloc(sizeof(sw_exception));
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

int main(int argc, char** argv) {
#if defined(_WIN32)
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
