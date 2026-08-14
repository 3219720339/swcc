// 动态加载 swgreet.dll：不链接导入库，运行时 LoadLibrary + GetProcAddress。
// 编译（Windows，MinGW/clang）：
//   clang dynamic-load.c -o dynamic-load.exe
// 运行前把 swgreet.dll 放在当前目录（或 PATH）。
// Linux/macOS 对应 dlopen/dlsym（符号名同）。
#include <stdio.h>
#if defined(_WIN32)
#include <windows.h>
#else
#include <dlfcn.h>
#endif

// Sw 运行时对象布局（与 runtime.c 一致）。
typedef struct sw_string {
    char* data;
    long long len;
} sw_string;

typedef sw_string* (*fn_greet)(sw_string*);
typedef long long (*fn_double)(long long);

int main(void) {
#if defined(_WIN32)
    void* lib = LoadLibraryA("swgreet.dll");
#else
    void* lib = dlopen("./swgreet.so", RTLD_NOW);
#endif
    if (lib == NULL) {
        printf("无法加载 swgreet.dll/.so\n");
        return 1;
    }
#if defined(_WIN32)
    fn_greet greet = (fn_greet)GetProcAddress((HMODULE)lib, "greet");
    fn_double dbl = (fn_double)GetProcAddress((HMODULE)lib, "twice");
#else
    fn_greet greet = (fn_greet)dlsym(lib, "sw_fn_greeter_greet_s");
    fn_double dbl = (fn_double)dlsym(lib, "sw_fn_greeter_twice_i");
#endif
    if (greet == NULL || dbl == NULL) {
        printf("找不到导出符号\n");
        return 1;
    }
    sw_string name = { "Dynamic", 7 };
    sw_string* result = greet(&name);
    printf("greet = %s (len=%lld)\n", result->data, result->len);
    printf("twice(21) = %lld\n", dbl(21));
#if defined(_WIN32)
    FreeLibrary((HMODULE)lib);
#else
    dlclose(lib);
#endif
    return 0;
}
