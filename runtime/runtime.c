// Sw 运行时（v0.1 最小实现）
// 无系统头文件依赖：仅显式声明用到的 CRT 函数，可同时用 MSVC/MinGW 工具链编译。

typedef long long int64_t;
typedef unsigned long long uint64_t;
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

static char* sw_dup(const char* src, int64_t len) {
    char* copy = (char*)malloc((uint64_t)len + 1);
    if (len > 0) {
        memcpy(copy, src, (uint64_t)len);
    }
    copy[len] = 0;
    return copy;
}

sw_string* sw_string_from_literal(const char* data, int64_t len) {
    sw_string* string = (sw_string*)malloc(sizeof(sw_string));
    string->data = sw_dup(data, len);
    string->len = len;
    return string;
}

sw_string* sw_string_concat(sw_string* a, sw_string* b) {
    sw_string* string = (sw_string*)malloc(sizeof(sw_string));
    int64_t len = a->len + b->len;
    string->data = (char*)malloc((uint64_t)len + 1);
    if (a->len > 0) {
        memcpy(string->data, a->data, (uint64_t)a->len);
    }
    if (b->len > 0) {
        memcpy(string->data + a->len, b->data, (uint64_t)b->len);
    }
    string->data[len] = 0;
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

sw_array* sw_array_new(int64_t elem_size, int64_t count) {
    (void)elem_size;
    sw_array* array = (sw_array*)malloc(sizeof(sw_array));
    array->len = count;
    array->cap = count;
    array->data = calloc((uint64_t)count, 8);
    return array;
}

void sw_array_set(sw_array* array, int64_t index, int64_t value) {
    ((int64_t*)array->data)[index] = value;
}

void* sw_object_new(int64_t size) {
    return calloc(1, (uint64_t)size);
}

// ---------------------------------------------------------------------------
// 闭包：{ fn 指针, 环境槽数组 }
// ---------------------------------------------------------------------------

typedef struct {
    void* fn;
    void* env;
} sw_closure;

void* sw_closure_new(void* fn, int64_t env_slots) {
    sw_closure* closure = (sw_closure*)malloc(sizeof(sw_closure));
    closure->fn = fn;
    closure->env = calloc((uint64_t)env_slots, 8);
    return closure;
}

void sw_env_set(void* closure, int64_t slot, int64_t value) {
    ((int64_t*)(((sw_closure*)closure)->env))[slot] = value;
}

int64_t sw_env_get(void* closure, int64_t slot) {
    return ((int64_t*)(((sw_closure*)closure)->env))[slot];
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
