// runtime_ui.c — Sw 跨平台桌面 UI 平台层（P1：Win32 窗口 + 事件）。
// 设计见 docs/11-UI设计.md。接入模式与 runtime_audio.c 一致：平台条件编译，
// 非 Windows（或缺少平台头）编译为安全 stub。
//
// 铁律：
//   1. 窗口后端线程永不执行 Sw 代码——事件只翻译成 int 数据入队，Sw 经
//      sw_ui_pump / sw_ui_event_* 主动拉取（与 std/audio 同原则）。
//   2. 所有原生状态在 C 内存，Sw 侧只见 ptr<void> handle。
//   3. sw_ui_pump 的 timeout 总有界返回（协作式 safepoint GC 依赖此点）。

typedef long long int64_t;
typedef unsigned long long uint64_t;
typedef unsigned int uint32_t;
typedef short int16_t;

// 与 runtime.c 布局一致的 Sw 字符串（extern c 的 string 参数）。
typedef struct { char* data; int64_t len; } sw_string;

#define SW_UI_EVENT_CAP 256
#define SW_UI_EVENT_NONE 0
#define SW_UI_EVENT_CLOSE_REQUESTED 1
#define SW_UI_EVENT_RESIZED 2
#define SW_UI_EVENT_KEY 3
#define SW_UI_EVENT_CURSOR 4
#define SW_UI_EVENT_MOUSE_BUTTON 5
#define SW_UI_EVENT_MOUSE_WHEEL 6
#define SW_UI_EVENT_FOCUS 7
#define SW_UI_EVENT_SCALE_FACTOR 8
#define SW_UI_EVENT_REDRAW 9

#define SW_UI_ERR_NONE 0
#define SW_UI_ERR_PLATFORM 1
#define SW_UI_ERR_WINDOW 2

#define SW_UI_CURSOR_DEFAULT 0
#define SW_UI_CURSOR_HAND 1
#define SW_UI_CURSOR_TEXT 2
#define SW_UI_CURSOR_WAIT 3

#if defined(_WIN32)

#ifndef WINVER
#define WINVER 0x0601
#endif
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0601
#endif
#include <windows.h>
#include <stdlib.h>
#include <string.h>

// Win8.1+ 消息，头文件可能未定义（_WIN32_WINNT 只开到 0x0601）。
#ifndef WM_DPICHANGED
#define WM_DPICHANGED 0x02E0
#endif
#ifndef IDC_HAND
#define IDC_HAND MAKEINTRESOURCE(32649)
#endif

typedef struct {
    int64_t kind, a, b, c, d;
} sw_ui_event;

typedef struct {
    HWND hwnd;
    int64_t open;
    int64_t last_error;
    int64_t scale_x1000; // DPI ×1000
    int64_t cursor;
    sw_ui_event queue[SW_UI_EVENT_CAP];
    int head;
    int count;
    int64_t last_a, last_b, last_c, last_d; // 最近一次 poll 弹出的事件载荷
} sw_ui_window;

static void sw_ui_push_event(sw_ui_window* w, int64_t kind, int64_t a, int64_t b,
                             int64_t c, int64_t d) {
    if (w->count >= SW_UI_EVENT_CAP) {
        // 溢出丢弃最旧。
        w->head = (w->head + 1) % SW_UI_EVENT_CAP;
        w->count--;
    }
    int tail = (w->head + w->count) % SW_UI_EVENT_CAP;
    w->queue[tail].kind = kind;
    w->queue[tail].a = a;
    w->queue[tail].b = b;
    w->queue[tail].c = c;
    w->queue[tail].d = d;
    w->count++;
}

// DPI 感知：GetProcAddress 探测，缺失（Win7）降级。
static void sw_ui_enable_dpi_aware(void) {
    static int done = 0;
    if (done) return;
    done = 1;
    HMODULE user32 = GetModuleHandleW(L"user32.dll");
    // SetProcessDpiAwarenessContext（Win10 1703+）：DPI_AWARENESS_CONTEXT
    // 就是 HANDLE；PER_MONITOR_AWARE_V2 = -4。
    typedef BOOL(WINAPI* fn_ctx)(void*);
    fn_ctx ctx = (fn_ctx)(void*)GetProcAddress(user32, "SetProcessDpiAwarenessContext");
    if (ctx != NULL) {
        ctx((void*)(intptr_t)-4);
        return;
    }
    typedef BOOL(WINAPI* fn_legacy)(void);
    fn_legacy legacy = (fn_legacy)(void*)GetProcAddress(user32, "SetProcessDPIAware");
    if (legacy != NULL) {
        legacy();
    }
}

static int sw_ui_scale_of(HWND hwnd) {
    typedef UINT(WINAPI* fn_dpi)(HWND);
    fn_dpi dpi = (fn_dpi)(void*)GetProcAddress(GetModuleHandleW(L"user32.dll"), "GetDpiForWindow");
    UINT value = dpi != NULL ? dpi(hwnd) : 96;
    if (value == 0) value = 96;
    return (int)value;
}

// UTF-8（sw_string，可能不带 NUL）→ UTF-16 动态缓冲；返回 NULL 表示失败。
static wchar_t* sw_ui_utf8_to_wide(sw_string* text, int* out_len) {
    if (text == NULL || text->data == NULL || text->len < 0) return NULL;
    int need = MultiByteToWideChar(CP_UTF8, 0, text->data, (int)text->len, NULL, 0);
    if (need <= 0) return NULL;
    wchar_t* buf = (wchar_t*)malloc(((size_t)need + 1) * sizeof(wchar_t));
    if (buf == NULL) return NULL;
    MultiByteToWideChar(CP_UTF8, 0, text->data, (int)text->len, buf, need);
    buf[need] = 0;
    if (out_len != NULL) *out_len = need;
    return buf;
}

// 标准游标 ID（mingw 头里 IDC_* 是 LPSTR 版，LoadCursorW 需要 LPCWSTR 版）。
static const wchar_t* sw_ui_cursor_id(int64_t cursor) {
    switch (cursor) {
        case SW_UI_CURSOR_HAND: return (const wchar_t*)(uintptr_t)32649; // IDC_HAND
        case SW_UI_CURSOR_TEXT: return (const wchar_t*)(uintptr_t)32513; // IDC_IBEAM
        case SW_UI_CURSOR_WAIT: return (const wchar_t*)(uintptr_t)32514; // IDC_WAIT
        default: return (const wchar_t*)(uintptr_t)32512;                // IDC_ARROW
    }
}

static LRESULT CALLBACK sw_ui_wndproc(HWND hwnd, UINT msg, WPARAM wparam, LPARAM lparam) {
    sw_ui_window* w = (sw_ui_window*)GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    switch (msg) {
        case WM_CLOSE:
            if (w != NULL) {
                w->open = 0;
                sw_ui_push_event(w, SW_UI_EVENT_CLOSE_REQUESTED, 0, 0, 0, 0);
            }
            return 0;
        case WM_SIZE:
            if (w != NULL) {
                sw_ui_push_event(w, SW_UI_EVENT_RESIZED, (int64_t)(int16_t)LOWORD(lparam),
                                 (int64_t)(int16_t)HIWORD(lparam), 0, 0);
            }
            return 0;
        case WM_KEYDOWN:
        case WM_KEYUP: {
            if (w == NULL) break;
            int64_t down = (msg == WM_KEYDOWN) ? 1 : 0;
            int64_t repeat = (lparam & 0x40000000) ? 1 : 0;
            int64_t scancode = (lparam >> 16) & 0xFF;
            int64_t mods = 0;
            if (GetKeyState(VK_SHIFT) & 0x8000) mods |= 1;
            if (GetKeyState(VK_CONTROL) & 0x8000) mods |= 2;
            if (GetKeyState(VK_MENU) & 0x8000) mods |= 4;
            if ((GetKeyState(VK_LWIN) & 0x8000) || (GetKeyState(VK_RWIN) & 0x8000)) mods |= 8;
            sw_ui_push_event(w, SW_UI_EVENT_KEY, scancode, down, repeat, mods);
            return 0;
        }
        case WM_MOUSEMOVE:
            if (w != NULL) {
                sw_ui_push_event(w, SW_UI_EVENT_CURSOR, (int64_t)(int16_t)LOWORD(lparam),
                                 (int64_t)(int16_t)HIWORD(lparam), 0, 0);
            }
            return 0;
        case WM_LBUTTONDOWN:
        case WM_LBUTTONUP:
        case WM_RBUTTONDOWN:
        case WM_RBUTTONUP:
        case WM_MBUTTONDOWN:
        case WM_MBUTTONUP: {
            if (w == NULL) break;
            int64_t button = (msg == WM_LBUTTONDOWN || msg == WM_LBUTTONUP)   ? 1
                             : (msg == WM_RBUTTONDOWN || msg == WM_RBUTTONUP) ? 2
                                                                              : 3;
            int64_t down = (msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN || msg == WM_MBUTTONDOWN)
                               ? 1
                               : 0;
            sw_ui_push_event(w, SW_UI_EVENT_MOUSE_BUTTON, button, down, 0, 0);
            return 0;
        }
        case WM_MOUSEWHEEL:
            if (w != NULL) {
                sw_ui_push_event(w, SW_UI_EVENT_MOUSE_WHEEL, 0,
                                 (int64_t)(int16_t)HIWORD(wparam), 0, 0);
            }
            return 0;
        case WM_SETFOCUS:
            if (w != NULL) sw_ui_push_event(w, SW_UI_EVENT_FOCUS, 1, 0, 0, 0);
            return 0;
        case WM_KILLFOCUS:
            if (w != NULL) sw_ui_push_event(w, SW_UI_EVENT_FOCUS, 0, 0, 0, 0);
            return 0;
        case WM_DPICHANGED: {
            if (w == NULL) break;
            int dpi = HIWORD(wparam);
            if (dpi <= 0) dpi = 96;
            w->scale_x1000 = (int64_t)dpi * 1000 / 96;
            sw_ui_push_event(w, SW_UI_EVENT_SCALE_FACTOR, w->scale_x1000, 0, 0, 0);
            return 0;
        }
        case WM_SETCURSOR:
            if (w != NULL && LOWORD(lparam) == HTCLIENT) {
                SetCursor(LoadCursorW(NULL, sw_ui_cursor_id(w->cursor)));
                return 0;
            }
            break;
        default:
            break;
    }
    return DefWindowProcW(hwnd, msg, wparam, lparam);
}

// 由逻辑客户区尺寸换算窗口尺寸（非客户区随 DPI 缩放，按 96 DPI 计算后整体乘系数）。
static void sw_ui_window_size_for_client(int64_t client_w, int64_t client_h, int scale,
                                         int* out_w, int* out_h) {
    RECT rc = {0, 0, (int)client_w, (int)client_h};
    AdjustWindowRectEx(&rc, WS_OVERLAPPEDWINDOW, FALSE, 0);
    *out_w = (rc.right - rc.left) * scale / 1000;
    *out_h = (rc.bottom - rc.top) * scale / 1000;
}

void* sw_ui_create(sw_string* title, int64_t width, int64_t height) {
    sw_ui_enable_dpi_aware();
    HINSTANCE instance = GetModuleHandleW(NULL);
    WNDCLASSW existing;
    if (!GetClassInfoW(instance, L"SwUiWindow", &existing)) {
        WNDCLASSW wc;
        memset(&wc, 0, sizeof(wc));
        wc.style = CS_HREDRAW | CS_VREDRAW;
        wc.lpfnWndProc = sw_ui_wndproc;
        wc.hInstance = instance;
        wc.hCursor = LoadCursorW(NULL, sw_ui_cursor_id(SW_UI_CURSOR_DEFAULT));
        wc.lpszClassName = L"SwUiWindow";
        if (!RegisterClassW(&wc)) return NULL;
    }
    int free_title = 0;
    wchar_t* title_buf = sw_ui_utf8_to_wide(title, NULL);
    if (title_buf == NULL) {
        title_buf = (wchar_t*)L"";
    } else {
        free_title = 1;
    }
    HWND hwnd = CreateWindowExW(0, L"SwUiWindow", title_buf,
                                WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, CW_USEDEFAULT,
                                (int)width, (int)height, NULL, NULL, instance, NULL);
    if (free_title) free(title_buf);
    if (hwnd == NULL) return NULL;
    sw_ui_window* w = (sw_ui_window*)calloc(1, sizeof(sw_ui_window));
    if (w == NULL) {
        DestroyWindow(hwnd);
        return NULL;
    }
    w->hwnd = hwnd;
    w->open = 1;
    w->last_error = SW_UI_ERR_NONE;
    w->scale_x1000 = (int64_t)sw_ui_scale_of(hwnd) * 1000 / 96;
    w->cursor = SW_UI_CURSOR_DEFAULT;
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, (LONG_PTR)w);
    // 客户区 = 请求的逻辑尺寸（物理像素 = 逻辑 × scale）。
    int win_w = 0, win_h = 0;
    sw_ui_window_size_for_client(width, height, (int)w->scale_x1000, &win_w, &win_h);
    SetWindowPos(hwnd, NULL, 0, 0, win_w, win_h, SWP_NOMOVE | SWP_NOZORDER);
    ShowWindow(hwnd, SW_SHOW);
    UpdateWindow(hwnd);
    return w;
}

void sw_ui_destroy(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w == NULL) return;
    DestroyWindow(w->hwnd);
    free(w);
}

int64_t sw_ui_is_open(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    return w != NULL ? w->open : 0;
}

void sw_ui_close(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w != NULL) PostMessageW(w->hwnd, WM_CLOSE, 0, 0);
}

void sw_ui_set_title(void* handle, sw_string* title) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w == NULL) return;
    wchar_t* buf = sw_ui_utf8_to_wide(title, NULL);
    if (buf == NULL) return;
    SetWindowTextW(w->hwnd, buf);
    free(buf);
}

void sw_ui_set_size(void* handle, int64_t width, int64_t height) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w == NULL) return;
    if (width < 1) width = 1;
    if (height < 1) height = 1;
    int win_w = 0, win_h = 0;
    sw_ui_window_size_for_client(width, height, (int)w->scale_x1000, &win_w, &win_h);
    SetWindowPos(w->hwnd, NULL, 0, 0, win_w, win_h, SWP_NOMOVE | SWP_NOZORDER);
}

void sw_ui_request_redraw(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w != NULL) InvalidateRect(w->hwnd, NULL, TRUE);
}

int64_t sw_ui_pump(void* handle, int64_t timeout_ms) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w == NULL) return 0;
    int64_t before = (int64_t)w->count;
    MSG msg;
    int waited = 0;
    for (;;) {
        if (PeekMessageW(&msg, NULL, 0, 0, PM_REMOVE)) {
            if (msg.message == WM_QUIT) {
                w->open = 0;
                sw_ui_push_event(w, SW_UI_EVENT_CLOSE_REQUESTED, 0, 0, 0, 0);
            } else {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            continue;
        }
        if (waited) break;
        DWORD ms = (DWORD)(timeout_ms < 0 ? 0
                                          : (timeout_ms > 0x7FFFFFFF ? 0x7FFFFFFF : timeout_ms));
        DWORD r = MsgWaitForMultipleObjectsEx(0, NULL, ms, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        if (r != WAIT_OBJECT_0) break; // 超时或错误：本次 pump 结束
        waited = 1;
    }
    return (int64_t)w->count - before;
}

int64_t sw_ui_event_poll(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w == NULL || w->count <= 0) return SW_UI_EVENT_NONE;
    sw_ui_event* e = &w->queue[w->head];
    w->last_a = e->a;
    w->last_b = e->b;
    w->last_c = e->c;
    w->last_d = e->d;
    w->head = (w->head + 1) % SW_UI_EVENT_CAP;
    w->count--;
    return e->kind;
}

int64_t sw_ui_event_a(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    return w != NULL ? w->last_a : 0;
}
int64_t sw_ui_event_b(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    return w != NULL ? w->last_b : 0;
}
int64_t sw_ui_event_c(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    return w != NULL ? w->last_c : 0;
}
int64_t sw_ui_event_d(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    return w != NULL ? w->last_d : 0;
}

int64_t sw_ui_get_scale(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    return w != NULL ? w->scale_x1000 : 1000;
}

void sw_ui_set_cursor(void* handle, int64_t cursor) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w == NULL) return;
    if (cursor < SW_UI_CURSOR_DEFAULT || cursor > SW_UI_CURSOR_WAIT) cursor = SW_UI_CURSOR_DEFAULT;
    w->cursor = cursor;
    SetCursor(LoadCursorW(NULL, sw_ui_cursor_id(cursor)));
}

int64_t sw_ui_last_error(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    return w != NULL ? w->last_error : SW_UI_ERR_PLATFORM;
}

#else // 非 Windows：安全 stub（musl 交叉编译等场景自动退化为不可用）。

void* sw_ui_create(sw_string* title, int64_t width, int64_t height) {
    (void)title;
    (void)width;
    (void)height;
    return NULL;
}
void sw_ui_destroy(void* handle) { (void)handle; }
int64_t sw_ui_is_open(void* handle) {
    (void)handle;
    return 0;
}
void sw_ui_close(void* handle) { (void)handle; }
void sw_ui_set_title(void* handle, sw_string* title) {
    (void)handle;
    (void)title;
}
void sw_ui_set_size(void* handle, int64_t width, int64_t height) {
    (void)handle;
    (void)width;
    (void)height;
}
void sw_ui_request_redraw(void* handle) { (void)handle; }
int64_t sw_ui_pump(void* handle, int64_t timeout_ms) {
    (void)handle;
    (void)timeout_ms;
    return 0;
}
int64_t sw_ui_event_poll(void* handle) {
    (void)handle;
    return SW_UI_EVENT_NONE;
}
int64_t sw_ui_event_a(void* handle) {
    (void)handle;
    return 0;
}
int64_t sw_ui_event_b(void* handle) {
    (void)handle;
    return 0;
}
int64_t sw_ui_event_c(void* handle) {
    (void)handle;
    return 0;
}
int64_t sw_ui_event_d(void* handle) {
    (void)handle;
    return 0;
}
int64_t sw_ui_get_scale(void* handle) {
    (void)handle;
    return 1000;
}
void sw_ui_set_cursor(void* handle, int64_t cursor) {
    (void)handle;
    (void)cursor;
}
int64_t sw_ui_last_error(void* handle) {
    (void)handle;
    return SW_UI_ERR_PLATFORM;
}

#endif
