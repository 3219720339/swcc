#ifndef SW_swmath_H
#define SW_swmath_H

// 由 swc 生成：Sw 模块导出的 C 接口（符号名为 Sw stable 名，
// 源码函数名见末尾注释）。
typedef struct sw_string { char* data; long long len; } sw_string;
typedef struct sw_array { long long len; long long cap; void* data; } sw_array;

extern long long sw_fn_math_add_ii(long long a, long long b);  // add
extern long long sw_fn_math_mul_ii(long long a, long long b);  // mul
extern long long sw_fn_math_square_i(long long x);  // square

#endif // SW_swmath_H
