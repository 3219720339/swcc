// runtime_ui.c — Sw 跨平台桌面 UI 平台层。
// P1：Win32 窗口 + 事件；P2：自绘 CPU 光栅化（圆角/渐变/文字）+
// frameless 窗口（自定义标题栏拖动/边缘缩放）+ 主屏居中 + DPI。
// 设计见 docs/11-UI设计.md；接入模式与 runtime_audio.c 一致。
//
// 铁律：
//   1. 窗口后端线程永不执行 Sw 代码——事件/绘制命令全部数据化。
//   2. 所有原生状态在 C 内存，Sw 侧只见 ptr<void> handle；绘制命令中的
//      文本在追加时拷贝进 C 内存（Sw 字符串可能被 GC 回收）。
//   3. sw_ui_pump 的 timeout 总有界返回（协作式 safepoint GC 依赖此点）。

typedef long long int64_t;
typedef unsigned long long uint64_t;
typedef unsigned int uint32_t;
typedef unsigned char uint8_t;
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
#define SW_UI_ERR_RENDERER 3

#define SW_UI_CURSOR_DEFAULT 0
#define SW_UI_CURSOR_HAND 1
#define SW_UI_CURSOR_TEXT 2
#define SW_UI_CURSOR_WAIT 3

#define SW_UI_CMD_CLEAR 1
#define SW_UI_CMD_FILL_RECT 2
#define SW_UI_CMD_FILL_ROUND_RECT 3
#define SW_UI_CMD_STROKE_ROUND_RECT 4
#define SW_UI_CMD_FILL_CIRCLE 5
#define SW_UI_CMD_LINEAR_GRADIENT 6
#define SW_UI_CMD_DRAW_TEXT 7

#define SW_UI_CMD_CAP 65536

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

#define STB_TRUETYPE_IMPLEMENTATION
// 无系统头环境：stb 的断言映射为空操作（避免链接 _assert）。
#ifndef STBTT_assert
#define STBTT_assert(x) ((void)0)
#endif
#include "vendor/stb/stb_truetype.h"

#ifndef WM_DPICHANGED
#define WM_DPICHANGED 0x02E0
#endif

typedef struct {
    int64_t kind, a, b, c, d;
} sw_ui_event;

// 绘制命令（几何 + 颜色；文本另行拷贝到 C 内存）。
typedef struct {
    uint8_t kind;
    uint8_t pad[7];
    float x, y, w, h, r, stroke;
    float gx0, gy0, gx1, gy1; // 渐变端点（相对矩形左上角）
    uint32_t c0, c1;
    float text_size;
    char* text;
} sw_ui_cmd;

typedef struct {
    HWND hwnd;
    int64_t open;
    int64_t last_error;
    int64_t scale_x1000; // DPI ×1000
    int64_t cursor;
    int64_t title_bar_h; // 标题栏拖动区域高度（逻辑像素）
    int64_t frameless;   // 无边框（自定义标题栏）模式
    int64_t client_w, client_h; // 逻辑客户区
    sw_ui_event queue[SW_UI_EVENT_CAP];
    int head;
    int count;
    int64_t last_a, last_b, last_c, last_d;

    // 帧缓冲（物理像素，DIB section）
    HBITMAP fb_bitmap;
    HDC fb_dc;
    void* fb_bits;
    int fb_w, fb_h;
    BITMAPINFO fb_info;

    // 绘制命令缓冲
    sw_ui_cmd* cmds;
    int cmd_count;
    int cmd_cap;
} sw_ui_window;

// 字体（进程级加载一次）
static unsigned char* g_font_data = NULL;
static stbtt_fontinfo g_font;
static int g_font_ok = 0;

static void sw_ui_load_font(void) {
    if (g_font_ok) return;
    static const wchar_t* candidates[] = {
        L"C:\\Windows\\Fonts\\msyh.ttc",
        L"C:\\Windows\\Fonts\\msyh.ttf",
        L"C:\\Windows\\Fonts\\simhei.ttf",
    };
    for (int i = 0; i < 3; i++) {
        HANDLE f = CreateFileW(candidates[i], GENERIC_READ, FILE_SHARE_READ, NULL,
                               OPEN_EXISTING, 0, NULL);
        if (f == INVALID_HANDLE_VALUE) continue;
        LARGE_INTEGER sz;
        if (!GetFileSizeEx(f, &sz) || sz.QuadPart <= 0 || sz.QuadPart > (64 * 1024 * 1024)) {
            CloseHandle(f);
            continue;
        }
        g_font_data = (unsigned char*)malloc((size_t)sz.QuadPart);
        DWORD read = 0;
        if (g_font_data != NULL && ReadFile(f, g_font_data, (DWORD)sz.QuadPart, &read, NULL) &&
            read == (DWORD)sz.QuadPart) {
            int offset = stbtt_GetFontOffsetForIndex(g_font_data, 0);
            g_font_ok = offset >= 0 && stbtt_InitFont(&g_font, g_font_data, offset);
        }
        CloseHandle(f);
        if (g_font_ok) return;
    }
}

static void sw_ui_push_event(sw_ui_window* w, int64_t kind, int64_t a, int64_t b,
                             int64_t c, int64_t d) {
    if (w->count >= SW_UI_EVENT_CAP) {
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

static void sw_ui_enable_dpi_aware(void) {
    static int done = 0;
    if (done) return;
    done = 1;
    HMODULE user32 = GetModuleHandleW(L"user32.dll");
    typedef BOOL(WINAPI* fn_ctx)(void*);
    fn_ctx ctx = (fn_ctx)(void*)GetProcAddress(user32, "SetProcessDpiAwarenessContext");
    if (ctx != NULL) {
        ctx((void*)(intptr_t)-4); // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
        return;
    }
    typedef BOOL(WINAPI* fn_legacy)(void);
    fn_legacy legacy = (fn_legacy)(void*)GetProcAddress(user32, "SetProcessDPIAware");
    if (legacy != NULL) legacy();
}

static int sw_ui_scale_of(HWND hwnd) {
    typedef UINT(WINAPI* fn_dpi)(HWND);
    fn_dpi dpi = (fn_dpi)(void*)GetProcAddress(GetModuleHandleW(L"user32.dll"), "GetDpiForWindow");
    UINT value = dpi != NULL ? dpi(hwnd) : 96;
    if (value == 0) value = 96;
    return (int)value;
}

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

// 由逻辑客户区尺寸换算窗口尺寸（非客户区随 DPI 缩放）。
static void sw_ui_window_size_for_client_ex(int64_t client_w, int64_t client_h, int scale,
                                            int frameless, int* out_w, int* out_h) {
    DWORD style = frameless ? (WS_POPUP | WS_THICKFRAME) : WS_OVERLAPPEDWINDOW;
    RECT rc = {0, 0, (int)client_w, (int)client_h};
    AdjustWindowRectEx(&rc, style, FALSE, 0);
    *out_w = (rc.right - rc.left) * scale / 1000;
    *out_h = (rc.bottom - rc.top) * scale / 1000;
}

static void sw_ui_window_size_for_client(int64_t client_w, int64_t client_h, int scale,
                                         int* out_w, int* out_h) {
    sw_ui_window_size_for_client_ex(client_w, client_h, scale, 0, out_w, out_h);
}

// 无边框窗口的现代观感：圆角（Win11）+ 阴影（Win10+），GetProcAddress 探测，
// 缺失（Win7）自动跳过。
static void sw_ui_apply_modern_style(HWND hwnd) {
    HMODULE dwm = LoadLibraryW(L"dwmapi.dll");
    if (dwm == NULL) return;
    typedef HRESULT(WINAPI* fn_attr)(HWND, DWORD, LPCVOID, DWORD);
    fn_attr attr = (fn_attr)(void*)GetProcAddress(dwm, "DwmSetWindowAttribute");
    if (attr != NULL) {
        DWORD preference = 2; // DWMWCP_ROUND
        attr(hwnd, 33 /* DWMWA_WINDOW_CORNER_PREFERENCE */, &preference, sizeof(preference));
    }
    typedef HRESULT(WINAPI* fn_extend)(HWND, const void*);
    fn_extend extend = (fn_extend)(void*)GetProcAddress(dwm, "DwmExtendFrameIntoClientArea");
    if (extend != NULL) {
        // DWM MARGINS 布局（mingw 头未必暴露 MARGINS，直接按布局声明）。
        int margins[4] = {-1, -1, -1, -1};
        extend(hwnd, margins);
    }
}

static const wchar_t* sw_ui_cursor_id(int64_t cursor) {
    switch (cursor) {
        case SW_UI_CURSOR_HAND: return (const wchar_t*)(uintptr_t)32649;
        case SW_UI_CURSOR_TEXT: return (const wchar_t*)(uintptr_t)32513;
        case SW_UI_CURSOR_WAIT: return (const wchar_t*)(uintptr_t)32514;
        default: return (const wchar_t*)(uintptr_t)32512;
    }
}

// ---------------------------------------------------------------------------
// 帧缓冲与光栅化（CPU，物理像素）
// ---------------------------------------------------------------------------

static void sw_ui_fb_resize(sw_ui_window* w, int phys_w, int phys_h) {
    if (w->fb_dc != NULL) {
        DeleteDC(w->fb_dc);
        w->fb_dc = NULL;
    }
    if (w->fb_bitmap != NULL) {
        DeleteObject(w->fb_bitmap);
        w->fb_bitmap = NULL;
    }
    w->fb_bits = NULL;
    w->fb_w = 0;
    w->fb_h = 0;
    if (phys_w < 1) phys_w = 1;
    if (phys_h < 1) phys_h = 1;
    HDC screen = GetDC(NULL);
    if (screen == NULL) return;
    w->fb_dc = CreateCompatibleDC(screen);
    ReleaseDC(NULL, screen);
    if (w->fb_dc == NULL) return;
    memset(&w->fb_info, 0, sizeof(w->fb_info));
    w->fb_info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    w->fb_info.bmiHeader.biWidth = phys_w;
    w->fb_info.bmiHeader.biHeight = -phys_h; // top-down
    w->fb_info.bmiHeader.biPlanes = 1;
    w->fb_info.bmiHeader.biBitCount = 32;
    w->fb_info.bmiHeader.biCompression = BI_RGB;
    w->fb_bitmap = CreateDIBSection(w->fb_dc, &w->fb_info, DIB_RGB_COLORS, &w->fb_bits, NULL, 0);
    if (w->fb_bitmap != NULL) {
        SelectObject(w->fb_dc, w->fb_bitmap);
        w->fb_w = phys_w;
        w->fb_h = phys_h;
    }
}

static inline void sw_ui_blend_px(uint32_t* dst, uint32_t src) {
    uint32_t sa = (src >> 24) & 0xFF;
    if (sa == 0) return;
    if (sa == 255) {
        *dst = src;
        return;
    }
    uint32_t dr = (*dst >> 16) & 0xFF, dg = (*dst >> 8) & 0xFF, db = *dst & 0xFF;
    uint32_t sr = (src >> 16) & 0xFF, sg = (src >> 8) & 0xFF, sb = src & 0xFF;
    uint32_t ia = 255 - sa;
    dr = (sr * sa + dr * ia) / 255;
    dg = (sg * sa + dg * ia) / 255;
    db = (sb * sa + db * ia) / 255;
    *dst = 0xFF000000 | (dr << 16) | (dg << 8) | db;
}

static void sw_ui_fill_rect(uint32_t* fb, int fb_w, int fb_h, float x, float y, float w,
                            float h, uint32_t color) {
    int x0 = (int)(x + 0.5f), y0 = (int)(y + 0.5f);
    int x1 = (int)(x + w + 0.5f), y1 = (int)(y + h + 0.5f);
    if (x0 < 0) x0 = 0;
    if (y0 < 0) y0 = 0;
    if (x1 > fb_w) x1 = fb_w;
    if (y1 > fb_h) y1 = fb_h;
    for (int py = y0; py < y1; py++) {
        uint32_t* row = fb + (size_t)py * fb_w;
        for (int px = x0; px < x1; px++) {
            sw_ui_blend_px(&row[px], color);
        }
    }
}

// 圆角矩形内含测试（含圆角，半像素内返回部分覆盖）。
static float sw_ui_round_rect_coverage(float px, float py, float x, float y, float w,
                                       float h, float r) {
    float cx = px - x, cy = py - y;
    if (cx < 0 || cx >= w || cy < 0 || cy >= h) return 0.0f;
    if (r <= 0.0f) return 1.0f;
    float rx = w - r, ry = h - r;
    float ox = 0.0f, oy = 0.0f;
    if (cx < r && cy < r) {
        ox = r - cx;
        oy = r - cy;
    } else if (cx > rx && cy < r) {
        ox = cx - rx;
        oy = r - cy;
    } else if (cx < r && cy > ry) {
        ox = r - cx;
        oy = cy - ry;
    } else if (cx > rx && cy > ry) {
        ox = cx - rx;
        oy = cy - ry;
    } else {
        return 1.0f;
    }
    float d = ox * ox + oy * oy;
    float rr = r * r;
    if (d > rr) return 0.0f;
    return 1.0f; // 圆角内部；AA 由 2×2 超采样提供
}

static void sw_ui_fill_round_rect(uint32_t* fb, int fb_w, int fb_h, float x, float y,
                                  float w, float h, float r, uint32_t color) {
    int x0 = (int)x, y0 = (int)y;
    int x1 = (int)(x + w) + 1, y1 = (int)(y + h) + 1;
    if (x0 < 0) x0 = 0;
    if (y0 < 0) y0 = 0;
    if (x1 > fb_w) x1 = fb_w;
    if (y1 > fb_h) y1 = fb_h;
    for (int py = y0; py < y1; py++) {
        uint32_t* row = fb + (size_t)py * fb_w;
        for (int px = x0; px < x1; px++) {
            // 2×2 超采样 AA
            float cov = 0.0f;
            cov += sw_ui_round_rect_coverage(px + 0.25f, py + 0.25f, x, y, w, h, r);
            cov += sw_ui_round_rect_coverage(px + 0.75f, py + 0.25f, x, y, w, h, r);
            cov += sw_ui_round_rect_coverage(px + 0.25f, py + 0.75f, x, y, w, h, r);
            cov += sw_ui_round_rect_coverage(px + 0.75f, py + 0.75f, x, y, w, h, r);
            cov *= 0.25f;
            if (cov <= 0.0f) continue;
            uint32_t sa = (uint32_t)(((color >> 24) & 0xFF) * cov);
            uint32_t c = (color & 0x00FFFFFF) | (sa << 24);
            sw_ui_blend_px(&row[px], c);
        }
    }
}

static void sw_ui_stroke_round_rect(uint32_t* fb, int fb_w, int fb_h, float x, float y,
                                    float w, float h, float r, float stroke, uint32_t color) {
    // 外框与内框（含圆角）之间的环带，2×2 超采样。
    float in_r = r - stroke;
    if (in_r < 0.0f) in_r = 0.0f;
    int x0 = (int)x, y0 = (int)y;
    int x1 = (int)(x + w) + 1, y1 = (int)(y + h) + 1;
    if (x0 < 0) x0 = 0;
    if (y0 < 0) y0 = 0;
    if (x1 > fb_w) x1 = fb_w;
    if (y1 > fb_h) y1 = fb_h;
    for (int py = y0; py < y1; py++) {
        uint32_t* row = fb + (size_t)py * fb_w;
        for (int px = x0; px < x1; px++) {
            float cov = 0.0f;
            for (int sy = 0; sy < 2; sy++) {
                for (int sx = 0; sx < 2; sx++) {
                    float qx = px + (sx ? 0.75f : 0.25f);
                    float qy = py + (sy ? 0.75f : 0.25f);
                    float outer = sw_ui_round_rect_coverage(qx, qy, x, y, w, h, r);
                    float inner = sw_ui_round_rect_coverage(qx, qy, x + stroke, y + stroke,
                                                            w - 2 * stroke, h - 2 * stroke,
                                                            in_r);
                    cov += outer * (1.0f - inner);
                }
            }
            cov *= 0.25f;
            if (cov <= 0.0f) continue;
            uint32_t sa = (uint32_t)(((color >> 24) & 0xFF) * cov);
            uint32_t c = (color & 0x00FFFFFF) | (sa << 24);
            sw_ui_blend_px(&row[px], c);
        }
    }
}

static float sw_ui_circle_coverage(float px, float py, float cx, float cy, float r) {
    float dx = px - cx, dy = py - cy;
    float d = dx * dx + dy * dy;
    if (d > r * r) return 0.0f;
    return 1.0f;
}

static void sw_ui_fill_circle(uint32_t* fb, int fb_w, int fb_h, float cx, float cy,
                              float r, uint32_t color) {
    int x0 = (int)(cx - r), y0 = (int)(cy - r);
    int x1 = (int)(cx + r) + 1, y1 = (int)(cy + r) + 1;
    if (x0 < 0) x0 = 0;
    if (y0 < 0) y0 = 0;
    if (x1 > fb_w) x1 = fb_w;
    if (y1 > fb_h) y1 = fb_h;
    for (int py = y0; py < y1; py++) {
        uint32_t* row = fb + (size_t)py * fb_w;
        for (int px = x0; px < x1; px++) {
            float cov = 0.0f;
            cov += sw_ui_circle_coverage(px + 0.25f, py + 0.25f, cx, cy, r);
            cov += sw_ui_circle_coverage(px + 0.75f, py + 0.25f, cx, cy, r);
            cov += sw_ui_circle_coverage(px + 0.25f, py + 0.75f, cx, cy, r);
            cov += sw_ui_circle_coverage(px + 0.75f, py + 0.75f, cx, cy, r);
            cov *= 0.25f;
            if (cov <= 0.0f) continue;
            uint32_t sa = (uint32_t)(((color >> 24) & 0xFF) * cov);
            uint32_t c = (color & 0x00FFFFFF) | (sa << 24);
            sw_ui_blend_px(&row[px], c);
        }
    }
}

static void sw_ui_fill_linear_gradient(uint32_t* fb, int fb_w, int fb_h, float x, float y,
                                       float w, float h, float x0, float y0, float x1,
                                       float y1, uint32_t c0, uint32_t c1) {
    float dx = x1 - x0, dy = y1 - y0;
    float len2 = dx * dx + dy * dy;
    if (len2 < 0.0001f) len2 = 0.0001f;
    int px0 = (int)x, py0 = (int)y;
    int px1 = (int)(x + w) + 1, py1 = (int)(y + h) + 1;
    if (px0 < 0) px0 = 0;
    if (py0 < 0) py0 = 0;
    if (px1 > fb_w) px1 = fb_w;
    if (py1 > fb_h) py1 = fb_h;
    uint32_t a0 = (c0 >> 24) & 0xFF, r0 = (c0 >> 16) & 0xFF, g0 = (c0 >> 8) & 0xFF, b0 = c0 & 0xFF;
    uint32_t a1 = (c1 >> 24) & 0xFF, r1 = (c1 >> 16) & 0xFF, g1 = (c1 >> 8) & 0xFF, b1 = c1 & 0xFF;
    for (int py = py0; py < py1; py++) {
        uint32_t* row = fb + (size_t)py * fb_w;
        for (int px = px0; px < px1; px++) {
            float t = ((px + 0.5f - x0) * dx + (py + 0.5f - y0) * dy) / len2;
            if (t < 0.0f) t = 0.0f;
            if (t > 1.0f) t = 1.0f;
            uint32_t c = (uint32_t)(a0 + (a1 - a0) * t) << 24 |
                         (uint32_t)(r0 + (r1 - r0) * t) << 16 |
                         (uint32_t)(g0 + (g1 - g0) * t) << 8 |
                         (uint32_t)(b0 + (b1 - b0) * t);
            sw_ui_blend_px(&row[px], c);
        }
    }
}

static int sw_ui_utf8_next(const unsigned char* s, int* cp) {
    unsigned char c = s[0];
    if (c < 0x80) {
        *cp = c;
        return 1;
    }
    if ((c & 0xE0) == 0xC0 && (s[1] & 0xC0) == 0x80) {
        *cp = ((c & 0x1F) << 6) | (s[1] & 0x3F);
        return 2;
    }
    if ((c & 0xF0) == 0xE0 && (s[1] & 0xC0) == 0x80 && (s[2] & 0xC0) == 0x80) {
        *cp = ((c & 0x0F) << 12) | ((s[1] & 0x3F) << 6) | (s[2] & 0x3F);
        return 3;
    }
    if ((c & 0xF8) == 0xF0 && (s[1] & 0xC0) == 0x80 && (s[2] & 0xC0) == 0x80 &&
        (s[3] & 0xC0) == 0x80) {
        *cp = ((c & 0x07) << 18) | ((s[1] & 0x3F) << 12) | ((s[2] & 0x3F) << 6) | (s[3] & 0x3F);
        return 4;
    }
    *cp = c;
    return 1;
}

static void sw_ui_draw_text_into(uint32_t* fb, int fb_w, int fb_h, float x, float y,
                                 float size, const char* text, uint32_t color, int scale) {
    if (!g_font_ok || text == NULL) return;
    float phys = size * scale / 1000.0f;
    if (phys < 1.0f) phys = 1.0f;
    float s = stbtt_ScaleForPixelHeight(&g_font, phys);
    int ascent = 0, descent = 0, linegap = 0;
    stbtt_GetFontVMetrics(&g_font, &ascent, &descent, &linegap);
    float baseline = y + ascent * s;
    float pen_x = x;
    const unsigned char* p = (const unsigned char*)text;
    while (*p != 0) {
        int cp = 0;
        int n = sw_ui_utf8_next(p, &cp);
        if (cp == '\n' || cp == '\r' || cp == '\t') {
            pen_x += 8.0f * size * scale / 1000.0f;
        } else {
            int glyph = stbtt_FindGlyphIndex(&g_font, cp);
            if (glyph != 0) {
                int adv = 0, lsb = 0;
                stbtt_GetGlyphHMetrics(&g_font, glyph, &adv, &lsb);
                int gw = 0, gh = 0, gox = 0, goy = 0;
                unsigned char* buf = stbtt_GetGlyphBitmap(&g_font, s, s, glyph, &gw, &gh,
                                                          &gox, &goy);
                if (buf != NULL) {
                    int dst_x = (int)(pen_x + gox);
                    int dst_y = (int)(baseline + goy);
                    for (int gy = 0; gy < gh; gy++) {
                        int py = dst_y + gy;
                        if (py < 0 || py >= fb_h) continue;
                        uint32_t* row = fb + (size_t)py * fb_w;
                        for (int gx = 0; gx < gw; gx++) {
                            int px = dst_x + gx;
                            if (px < 0 || px >= fb_w) continue;
                            uint32_t a = buf[(size_t)gy * gw + gx];
                            if (a == 0) continue;
                            uint32_t sa = ((color >> 24) & 0xFF) * a / 255;
                            uint32_t c = (color & 0x00FFFFFF) | (sa << 24);
                            sw_ui_blend_px(&row[px], c);
                        }
                    }
                    stbtt_FreeBitmap(buf, NULL);
                }
                pen_x += adv * s;
            }
        }
        p += n;
    }
}

// ---------------------------------------------------------------------------
// 命令缓冲
// ---------------------------------------------------------------------------

static void sw_ui_cmd_push(sw_ui_window* w, sw_ui_cmd* cmd) {
    if (w->cmd_count >= w->cmd_cap) {
        int cap = w->cmd_cap == 0 ? 256 : w->cmd_cap * 2;
        if (cap > SW_UI_CMD_CAP) cap = SW_UI_CMD_CAP;
        if (cap <= w->cmd_cap) {
            w->last_error = SW_UI_ERR_RENDERER;
            return;
        }
        sw_ui_cmd* grown = (sw_ui_cmd*)realloc(w->cmds, (size_t)cap * sizeof(sw_ui_cmd));
        if (grown == NULL) {
            w->last_error = SW_UI_ERR_RENDERER;
            return;
        }
        w->cmds = grown;
        w->cmd_cap = cap;
    }
    w->cmds[w->cmd_count++] = *cmd;
}

static void sw_ui_cmd_clear_all(sw_ui_window* w) {
    for (int i = 0; i < w->cmd_count; i++) {
        if (w->cmds[i].text != NULL) {
            free(w->cmds[i].text);
            w->cmds[i].text = NULL;
        }
    }
    w->cmd_count = 0;
}

static void sw_ui_clear_canvas_cmds(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w != NULL) sw_ui_cmd_clear_all(w);
}

// ---------------------------------------------------------------------------
// 窗口过程
// ---------------------------------------------------------------------------

static LRESULT CALLBACK sw_ui_wndproc(HWND hwnd, UINT msg, WPARAM wparam, LPARAM lparam) {
    sw_ui_window* w = (sw_ui_window*)GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    switch (msg) {
        case WM_CLOSE:
            if (w != NULL) {
                w->open = 0;
                sw_ui_push_event(w, SW_UI_EVENT_CLOSE_REQUESTED, 0, 0, 0, 0);
            }
            return 0;
        case WM_SIZE: {
            if (w == NULL) break;
            int phys_w = LOWORD(lparam), phys_h = HIWORD(lparam);
            if (w->scale_x1000 > 0) {
                w->client_w = (int64_t)phys_w * 1000 / w->scale_x1000;
                w->client_h = (int64_t)phys_h * 1000 / w->scale_x1000;
            }
            sw_ui_fb_resize(w, phys_w, phys_h);
            sw_ui_push_event(w, SW_UI_EVENT_RESIZED, (int64_t)phys_w, (int64_t)phys_h, 0, 0);
            return 0;
        }
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
            // 保持逻辑客户区不变，重算物理尺寸。
            int win_w = 0, win_h = 0;
            sw_ui_window_size_for_client_ex(w->client_w, w->client_h, (int)w->scale_x1000,
                                            (int)w->frameless, &win_w, &win_h);
            RECT* rc = (RECT*)lparam;
            SetWindowPos(hwnd, NULL, rc->left, rc->top, win_w, win_h,
                         SWP_NOZORDER | SWP_NOACTIVATE);
            sw_ui_push_event(w, SW_UI_EVENT_SCALE_FACTOR, w->scale_x1000, 0, 0, 0);
            return 0;
        }
        case WM_NCHITTEST: {
            // 仅无边框模式自定义命中测试；系统窗口交给 DefWindowProc
            // （原生标题栏拖动/按钮/缩放全部保留）。
            if (w == NULL || !w->frameless) break;
            POINT pt = {(int)(int16_t)LOWORD(lparam), (int)(int16_t)HIWORD(lparam)};
            ScreenToClient(hwnd, &pt);
            RECT rc;
            GetClientRect(hwnd, &rc);
            int edge = 6;
            int hit = HTCLIENT;
            int top = pt.y < edge;
            int bottom = pt.y >= rc.bottom - edge;
            int left = pt.x < edge;
            int right = pt.x >= rc.right - edge;
            if (top && left) hit = HTTOPLEFT;
            else if (top && right) hit = HTTOPRIGHT;
            else if (bottom && left) hit = HTBOTTOMLEFT;
            else if (bottom && right) hit = HTBOTTOMRIGHT;
            else if (top) hit = HTTOP;
            else if (bottom) hit = HTBOTTOM;
            else if (left) hit = HTLEFT;
            else if (right) hit = HTRIGHT;
            else if (w->title_bar_h > 0 && pt.y < w->title_bar_h * w->scale_x1000 / 1000)
                hit = HTCAPTION;
            return hit;
        }
        case WM_SETCURSOR:
            if (w != NULL && LOWORD(lparam) == HTCLIENT) {
                SetCursor(LoadCursorW(NULL, sw_ui_cursor_id(w->cursor)));
                return 0;
            }
            break;
        case WM_ERASEBKGND:
            return 1; // 避免闪烁
        case WM_PAINT: {
            PAINTSTRUCT ps;
            HDC dc = BeginPaint(hwnd, &ps);
            if (w != NULL && w->fb_dc != NULL && w->fb_bitmap != NULL) {
                BitBlt(dc, 0, 0, w->fb_w, w->fb_h, w->fb_dc, 0, 0, SRCCOPY);
            } else {
                RECT rc;
                GetClientRect(hwnd, &rc);
                FillRect(dc, &rc, (HBRUSH)GetStockObject(WHITE_BRUSH));
            }
            EndPaint(hwnd, &ps);
            return 0;
        }
        default:
            break;
    }
    return DefWindowProcW(hwnd, msg, wparam, lparam);
}

// ---------------------------------------------------------------------------
// 公开 API
// ---------------------------------------------------------------------------

void* sw_ui_create_ex(sw_string* title, int64_t width, int64_t height, int64_t frameless) {
    sw_ui_enable_dpi_aware();
    sw_ui_load_font();
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
    // 默认系统窗口（WS_OVERLAPPEDWINDOW 原生标题栏）；frameless 用
    // WS_POPUP + WS_THICKFRAME（可缩放、无系统标题栏、可加圆角/阴影）。
    DWORD style = frameless ? (WS_POPUP | WS_THICKFRAME) : WS_OVERLAPPEDWINDOW;
    HWND hwnd = CreateWindowExW(0, L"SwUiWindow", title_buf, style,
                                CW_USEDEFAULT, CW_USEDEFAULT, 200, 120, NULL, NULL, instance,
                                NULL);
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
    w->client_w = width;
    w->client_h = height;
    w->frameless = frameless != 0;
    if (w->frameless) sw_ui_apply_modern_style(hwnd);
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, (LONG_PTR)w);
    // 客户区 = 请求的逻辑尺寸；主屏工作区居中。
    int win_w = 0, win_h = 0;
    sw_ui_window_size_for_client_ex(width, height, (int)w->scale_x1000, (int)w->frameless,
                                    &win_w, &win_h);
    RECT work = {0, 0, 800, 600};
    SystemParametersInfoW(SPI_GETWORKAREA, 0, &work, 0);
    int x = work.left + ((work.right - work.left) - win_w) / 2;
    int y = work.top + ((work.bottom - work.top) - win_h) / 2;
    if (x < work.left) x = work.left;
    if (y < work.top) y = work.top;
    SetWindowPos(hwnd, NULL, x, y, win_w, win_h, SWP_NOZORDER);
    ShowWindow(hwnd, SW_SHOW);
    UpdateWindow(hwnd);
    return w;
}

// 默认系统窗口样式。
void* sw_ui_create(sw_string* title, int64_t width, int64_t height) {
    return sw_ui_create_ex(title, width, height, 0);
}

void sw_ui_minimize(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w != NULL) ShowWindow(w->hwnd, SW_MINIMIZE);
}

void sw_ui_maximize(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w != NULL) ShowWindow(w->hwnd, SW_MAXIMIZE);
}

void sw_ui_restore(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w != NULL) ShowWindow(w->hwnd, SW_RESTORE);
}

int64_t sw_ui_is_maximized(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    return w != NULL && IsZoomed(w->hwnd) ? 1 : 0;
}

void sw_ui_destroy(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w == NULL) return;
    sw_ui_cmd_clear_all(w);
    if (w->cmds != NULL) free(w->cmds);
    if (w->fb_dc != NULL) DeleteDC(w->fb_dc);
    if (w->fb_bitmap != NULL) DeleteObject(w->fb_bitmap);
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
    w->client_w = width;
    w->client_h = height;
    int win_w = 0, win_h = 0;
    sw_ui_window_size_for_client_ex(width, height, (int)w->scale_x1000, (int)w->frameless,
                                    &win_w, &win_h);
    SetWindowPos(w->hwnd, NULL, 0, 0, win_w, win_h, SWP_NOMOVE | SWP_NOZORDER);
}

void sw_ui_set_title_bar(void* handle, int64_t height) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w != NULL) w->title_bar_h = height;
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
        if (r != WAIT_OBJECT_0) break;
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

// —— 绘制命令（逻辑像素，present 时按 scale 放大）——

void sw_ui_canvas_clear(void* handle, uint32_t color) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w == NULL) return;
    sw_ui_cmd cmd;
    memset(&cmd, 0, sizeof(cmd));
    cmd.kind = SW_UI_CMD_CLEAR;
    cmd.c0 = color;
    sw_ui_cmd_push(w, &cmd);
}

void sw_ui_canvas_fill_rect(void* handle, double x, double y, double w, double h,
                            uint32_t color) {
    sw_ui_window* win = (sw_ui_window*)handle;
    if (win == NULL) return;
    sw_ui_cmd cmd;
    memset(&cmd, 0, sizeof(cmd));
    cmd.kind = SW_UI_CMD_FILL_RECT;
    cmd.x = (float)x;
    cmd.y = (float)y;
    cmd.w = (float)w;
    cmd.h = (float)h;
    cmd.c0 = color;
    sw_ui_cmd_push(win, &cmd);
}

void sw_ui_canvas_fill_round_rect(void* handle, double x, double y, double w, double h,
                                  double r, uint32_t color) {
    sw_ui_window* win = (sw_ui_window*)handle;
    if (win == NULL) return;
    sw_ui_cmd cmd;
    memset(&cmd, 0, sizeof(cmd));
    cmd.kind = SW_UI_CMD_FILL_ROUND_RECT;
    cmd.x = (float)x;
    cmd.y = (float)y;
    cmd.w = (float)w;
    cmd.h = (float)h;
    cmd.r = (float)r;
    cmd.c0 = color;
    sw_ui_cmd_push(win, &cmd);
}

void sw_ui_canvas_stroke_round_rect(void* handle, double x, double y, double w, double h,
                                    double r, double stroke_w, uint32_t color) {
    sw_ui_window* win = (sw_ui_window*)handle;
    if (win == NULL) return;
    sw_ui_cmd cmd;
    memset(&cmd, 0, sizeof(cmd));
    cmd.kind = SW_UI_CMD_STROKE_ROUND_RECT;
    cmd.x = (float)x;
    cmd.y = (float)y;
    cmd.w = (float)w;
    cmd.h = (float)h;
    cmd.r = (float)r;
    cmd.stroke = (float)stroke_w;
    cmd.c0 = color;
    sw_ui_cmd_push(win, &cmd);
}

void sw_ui_canvas_fill_circle(void* handle, double cx, double cy, double r, uint32_t color) {
    sw_ui_window* win = (sw_ui_window*)handle;
    if (win == NULL) return;
    sw_ui_cmd cmd;
    memset(&cmd, 0, sizeof(cmd));
    cmd.kind = SW_UI_CMD_FILL_CIRCLE;
    cmd.x = (float)cx;
    cmd.y = (float)cy;
    cmd.r = (float)r;
    cmd.c0 = color;
    sw_ui_cmd_push(win, &cmd);
}

void sw_ui_canvas_fill_linear_gradient(void* handle, double x, double y, double w, double h,
                                       double x0, double y0, double x1, double y1,
                                       uint32_t c0, uint32_t c1) {
    sw_ui_window* win = (sw_ui_window*)handle;
    if (win == NULL) return;
    sw_ui_cmd cmd;
    memset(&cmd, 0, sizeof(cmd));
    cmd.kind = SW_UI_CMD_LINEAR_GRADIENT;
    cmd.x = (float)x;
    cmd.y = (float)y;
    cmd.w = (float)w;
    cmd.h = (float)h;
    cmd.c0 = c0;
    cmd.c1 = c1;
    cmd.gx0 = (float)(x0 - x);
    cmd.gy0 = (float)(y0 - y);
    cmd.gx1 = (float)(x1 - x);
    cmd.gy1 = (float)(y1 - y);
    sw_ui_cmd_push(win, &cmd);
}

void sw_ui_canvas_draw_text(void* handle, double x, double y, double size, sw_string* text,
                            uint32_t color) {
    sw_ui_window* win = (sw_ui_window*)handle;
    if (win == NULL || text == NULL || text->data == NULL || text->len < 0) return;
    sw_ui_cmd cmd;
    memset(&cmd, 0, sizeof(cmd));
    cmd.kind = SW_UI_CMD_DRAW_TEXT;
    cmd.x = (float)x;
    cmd.y = (float)y;
    cmd.text_size = (float)size;
    cmd.c0 = color;
    if (text->len > 0) {
        char* copy = (char*)malloc((size_t)text->len + 1);
        if (copy == NULL) return;
        memcpy(copy, text->data, (size_t)text->len);
        copy[text->len] = 0;
        cmd.text = copy;
    }
    sw_ui_cmd_push(win, &cmd);
}

double sw_ui_canvas_text_width(void* handle, double size, sw_string* text) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w == NULL || !g_font_ok || text == NULL || text->data == NULL) return 0.0;
    float phys = (float)size * (float)w->scale_x1000 / 1000.0f;
    if (phys < 1.0f) phys = 1.0f;
    float s = stbtt_ScaleForPixelHeight(&g_font, phys);
    float pen = 0.0f;
    const unsigned char* p = (const unsigned char*)text->data;
    int remaining = (int)text->len;
    while (remaining > 0) {
        int cp = 0;
        int n = sw_ui_utf8_next(p, &cp);
        if (n > remaining) break;
        int glyph = stbtt_FindGlyphIndex(&g_font, cp);
        if (glyph != 0) {
            int adv = 0, lsb = 0;
            stbtt_GetGlyphHMetrics(&g_font, glyph, &adv, &lsb);
            pen += adv * s;
        }
        p += n;
        remaining -= n;
    }
    return (double)(pen * 1000.0 / (float)w->scale_x1000);
}

int64_t sw_ui_present(void* handle) {
    sw_ui_window* w = (sw_ui_window*)handle;
    if (w == NULL) return 1;
    if (w->fb_bits == NULL || w->fb_w <= 0 || w->fb_h <= 0) {
        sw_ui_cmd_clear_all(w);
        return 0;
    }
    uint32_t* fb = (uint32_t*)w->fb_bits;
    int fb_w = w->fb_w, fb_h = w->fb_h;
    int scale = (int)w->scale_x1000;
    // 清屏
    uint32_t bg = 0xFFF5F5F5;
    for (size_t i = 0; i < (size_t)fb_w * fb_h; i++) fb[i] = bg;
    // 重放命令（逻辑 → 物理 ×scale/1000）
    for (int i = 0; i < w->cmd_count; i++) {
        sw_ui_cmd* cmd = &w->cmds[i];
        float k = scale / 1000.0f;
        switch (cmd->kind) {
            case SW_UI_CMD_CLEAR:
                for (size_t j = 0; j < (size_t)fb_w * fb_h; j++) fb[j] = cmd->c0;
                break;
            case SW_UI_CMD_FILL_RECT:
                sw_ui_fill_rect(fb, fb_w, fb_h, cmd->x * k, cmd->y * k, cmd->w * k,
                                cmd->h * k, cmd->c0);
                break;
            case SW_UI_CMD_FILL_ROUND_RECT:
                sw_ui_fill_round_rect(fb, fb_w, fb_h, cmd->x * k, cmd->y * k, cmd->w * k,
                                      cmd->h * k, cmd->r * k, cmd->c0);
                break;
            case SW_UI_CMD_STROKE_ROUND_RECT:
                sw_ui_stroke_round_rect(fb, fb_w, fb_h, cmd->x * k, cmd->y * k, cmd->w * k,
                                        cmd->h * k, cmd->r * k, cmd->stroke * k, cmd->c0);
                break;
            case SW_UI_CMD_FILL_CIRCLE:
                sw_ui_fill_circle(fb, fb_w, fb_h, cmd->x * k, cmd->y * k, cmd->r * k, cmd->c0);
                break;
            case SW_UI_CMD_LINEAR_GRADIENT:
                sw_ui_fill_linear_gradient(fb, fb_w, fb_h, cmd->x * k, cmd->y * k,
                                           cmd->w * k, cmd->h * k,
                                           (cmd->gx0 + cmd->x) * k, (cmd->gy0 + cmd->y) * k,
                                           (cmd->gx1 + cmd->x) * k, (cmd->gy1 + cmd->y) * k,
                                           cmd->c0, cmd->c1);
                break;
            case SW_UI_CMD_DRAW_TEXT:
                sw_ui_draw_text_into(fb, fb_w, fb_h, cmd->x * k, cmd->y * k, cmd->text_size,
                                     cmd->text != NULL ? cmd->text : "", cmd->c0, scale);
                break;
            default:
                break;
        }
    }
    sw_ui_cmd_clear_all(w);
    InvalidateRect(w->hwnd, NULL, FALSE);
    UpdateWindow(w->hwnd);
    return 0;
}

#else // 非 Windows：安全 stub（musl 交叉编译等场景自动退化为不可用）。

void* sw_ui_create_ex(sw_string* title, int64_t width, int64_t height, int64_t frameless) {
    (void)title;
    (void)width;
    (void)height;
    (void)frameless;
    return NULL;
}
void* sw_ui_create(sw_string* title, int64_t width, int64_t height) {
    (void)title;
    (void)width;
    (void)height;
    return NULL;
}
void sw_ui_minimize(void* handle) { (void)handle; }
void sw_ui_maximize(void* handle) { (void)handle; }
void sw_ui_restore(void* handle) { (void)handle; }
int64_t sw_ui_is_maximized(void* handle) {
    (void)handle;
    return 0;
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
void sw_ui_set_title_bar(void* handle, int64_t height) {
    (void)handle;
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
void sw_ui_canvas_clear(void* handle, uint32_t color) {
    (void)handle;
    (void)color;
}
void sw_ui_canvas_fill_rect(void* handle, double x, double y, double w, double h,
                            uint32_t color) {
    (void)handle;
    (void)x;
    (void)y;
    (void)w;
    (void)h;
    (void)color;
}
void sw_ui_canvas_fill_round_rect(void* handle, double x, double y, double w, double h,
                                  double r, uint32_t color) {
    (void)handle;
    (void)x;
    (void)y;
    (void)w;
    (void)h;
    (void)r;
    (void)color;
}
void sw_ui_canvas_stroke_round_rect(void* handle, double x, double y, double w, double h,
                                    double r, double stroke_w, uint32_t color) {
    (void)handle;
    (void)x;
    (void)y;
    (void)w;
    (void)h;
    (void)r;
    (void)stroke_w;
    (void)color;
}
void sw_ui_canvas_fill_circle(void* handle, double cx, double cy, double r, uint32_t color) {
    (void)handle;
    (void)cx;
    (void)cy;
    (void)r;
    (void)color;
}
void sw_ui_canvas_fill_linear_gradient(void* handle, double x, double y, double w, double h,
                                       double x0, double y0, double x1, double y1,
                                       uint32_t c0, uint32_t c1) {
    (void)handle;
    (void)x;
    (void)y;
    (void)w;
    (void)h;
    (void)x0;
    (void)y0;
    (void)x1;
    (void)y1;
    (void)c0;
    (void)c1;
}
void sw_ui_canvas_draw_text(void* handle, double x, double y, double size, sw_string* text,
                            uint32_t color) {
    (void)handle;
    (void)x;
    (void)y;
    (void)size;
    (void)text;
    (void)color;
}
double sw_ui_canvas_text_width(void* handle, double size, sw_string* text) {
    (void)handle;
    (void)size;
    (void)text;
    return 0.0;
}
int64_t sw_ui_present(void* handle) {
    (void)handle;
    return 1;
}

#endif
