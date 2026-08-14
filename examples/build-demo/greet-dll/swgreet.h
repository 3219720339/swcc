#ifndef SW_swgreet_H
#define SW_swgreet_H

// 由 swc 生成：Sw 模块导出的 C 接口（按源码 `export function` 收集）。
typedef struct sw_string { char* data; long long len; } sw_string;
typedef struct sw_array { long long len; long long cap; void* data; } sw_array;

extern sw_string* greet(sw_string* name);
extern sw_string* repeat(sw_string* text, long long times);
extern long long twice(long long x);

#endif // SW_swgreet_H
