// Sw 运行时（v0.1 最小实现）
// ABI 约定：string/array/object 都以堆指针传递；标量统一 64 位。
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

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
    char* copy = (char*)malloc((size_t)len + 1);
    if (len > 0) {
        memcpy(copy, src, (size_t)len);
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
    string->data = (char*)malloc((size_t)len + 1);
    if (a->len > 0) {
        memcpy(string->data, a->data, (size_t)a->len);
    }
    if (b->len > 0) {
        memcpy(string->data + a->len, b->data, (size_t)b->len);
    }
    string->data[len] = 0;
    string->len = len;
    return string;
}

sw_string* sw_int_to_string(int64_t value) {
    char buffer[64];
    int len = snprintf(buffer, sizeof(buffer), "%lld", (long long)value);
    return sw_string_from_literal(buffer, len);
}

sw_string* sw_uint_to_string(uint64_t value) {
    char buffer[64];
    int len = snprintf(buffer, sizeof(buffer), "%llu", (unsigned long long)value);
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
        fwrite(string->data, 1, (size_t)string->len, stdout);
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
    array->data = calloc((size_t)count, 8);
    return array;
}

void sw_array_set(sw_array* array, int64_t index, int64_t value) {
    ((int64_t*)array->data)[index] = value;
}

void* sw_object_new(int64_t size) {
    return calloc(1, (size_t)size);
}
