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
extern int snprintf(char* buffer, sw_size size, const char* format, ...);
extern uint64_t fwrite(const void* data, sw_size size, sw_size count, void* stream);
extern int fputc(int character, void* stream);
#if defined(_WIN32)
extern void* __acrt_iob_func(unsigned int index);
#define stdout __acrt_iob_func(1)
#else
extern void* stdout;
#endif

// 汇编实现的 setjmp/longjmp（runtime.s）
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

// stdin：Windows 用 __acrt_iob_func(0)，POSIX 直接用 stdin。
#if defined(_WIN32)
#define stdin __acrt_iob_func(0)
#else
extern void* stdin;
#endif

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

int64_t abs(int64_t value) {
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

int64_t open(sw_string* path, sw_string* mode) {
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

int64_t close(int64_t fd) {
    if (fd < 0 || fd >= SW_MAX_FILES || sw_files[fd] == NULL) {
        return -1;
    }
    int result = fclose(sw_files[fd]);
    sw_files[fd] = NULL;
    return result == 0 ? 0 : -1;
}

int64_t write(int64_t fd, sw_string* text) {
    if (fd < 0 || fd >= SW_MAX_FILES || sw_files[fd] == NULL) {
        return -1;
    }
    return (int64_t)fwrite(text->data, 1, (uint64_t)text->len, sw_files[fd]);
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

// ---------------------------------------------------------------------------
// string：字节语义的查找/子串（UTF-8 边界由用户保证，与 .length 一致）。
// ---------------------------------------------------------------------------

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

sw_array* sw_array_new(int64_t elem_size, int64_t count) {
    (void)elem_size;
    if (count < 0) {
        count = 0;
    }
    sw_array* array =
        (sw_array*)sw_gc_alloc(sizeof(sw_array) + (uint64_t)count * 8);
    array->len = count;
    array->cap = count;
    array->data = (void*)(array + 1);
    memset(array->data, 0, (sw_size)((uint64_t)count * 8));
    return array;
}

void sw_array_set(sw_array* array, int64_t index, int64_t value) {
    ((int64_t*)array->data)[index] = value;
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
