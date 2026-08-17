// std/ui - Sw 跨平台桌面 UI（P1：窗口 + 事件）。
// 设计见 docs/11-UI设计.md。事件数据化：窗口后端线程永不回调 Sw 代码，
// 事件由 Application.pump() 收集、poll() 拉取（与 std/audio 同原则）；
// 所有 setter 返回 this 支持链式调用。

export extern c function sw_ui_create(title: string, width: int, height: int): ptr<void>;
export extern c function sw_ui_destroy(handle: ptr<void>): void;
export extern c function sw_ui_is_open(handle: ptr<void>): int;
export extern c function sw_ui_close(handle: ptr<void>): void;
export extern c function sw_ui_set_title(handle: ptr<void>, title: string): void;
export extern c function sw_ui_set_size(handle: ptr<void>, width: int, height: int): void;
export extern c function sw_ui_request_redraw(handle: ptr<void>): void;
export extern c function sw_ui_pump(handle: ptr<void>, timeout_ms: int): int;
export extern c function sw_ui_event_poll(handle: ptr<void>): int;
export extern c function sw_ui_event_a(handle: ptr<void>): int;
export extern c function sw_ui_event_b(handle: ptr<void>): int;
export extern c function sw_ui_event_c(handle: ptr<void>): int;
export extern c function sw_ui_event_d(handle: ptr<void>): int;
export extern c function sw_ui_get_scale(handle: ptr<void>): int;
export extern c function sw_ui_set_cursor(handle: ptr<void>, cursor: int): void;
export extern c function sw_ui_set_title_bar(handle: ptr<void>, height: int): void;
export extern c function sw_ui_last_error(handle: ptr<void>): int;
export extern c function sw_ui_present(handle: ptr<void>): int;
export extern c function sw_ui_canvas_clear(handle: ptr<void>, color: int): void;
export extern c function sw_ui_canvas_fill_rect(handle: ptr<void>, x: float, y: float, w: float, h: float, color: int): void;
export extern c function sw_ui_canvas_fill_round_rect(handle: ptr<void>, x: float, y: float, w: float, h: float, r: float, color: int): void;
export extern c function sw_ui_canvas_stroke_round_rect(handle: ptr<void>, x: float, y: float, w: float, h: float, r: float, stroke_w: float, color: int): void;
export extern c function sw_ui_canvas_fill_circle(handle: ptr<void>, cx: float, cy: float, r: float, color: int): void;
export extern c function sw_ui_canvas_fill_linear_gradient(handle: ptr<void>, x: float, y: float, w: float, h: float, x0: float, y0: float, x1: float, y1: float, c0: int, c1: int): void;
export extern c function sw_ui_canvas_draw_text(handle: ptr<void>, x: float, y: float, size: float, text: string, color: int): void;
export extern c function sw_ui_canvas_text_width(handle: ptr<void>, size: float, text: string): float;

/// 当前没有待处理事件。
export const UI_EVENT_NONE = 0;
/// 用户或系统请求关闭窗口；收到后主循环应自行保存状态并退出。
export const UI_EVENT_CLOSE_REQUESTED = 1;
/// 窗口尺寸变化；a=新宽，b=新高（物理像素）。
export const UI_EVENT_RESIZED = 2;
/// 物理键盘事件；a=scancode，b=是否按下，c=是否自动重复，d=修饰位掩码
/// （bit0 Shift、bit1 Ctrl、bit2 Alt、bit3 Super）。
export const UI_EVENT_KEY = 3;
/// 鼠标移动；a=x，b=y（物理像素）。
export const UI_EVENT_CURSOR = 4;
/// 鼠标按键；a=1 左键/2 右键/3 中键，b=是否按下。
export const UI_EVENT_MOUSE_BUTTON = 5;
/// 鼠标滚轮；a=横向增量，b=纵向增量（一格 120）。
export const UI_EVENT_MOUSE_WHEEL = 6;
/// 焦点变化；a=1 获得焦点，a=0 失去焦点。
export const UI_EVENT_FOCUS = 7;
/// DPI 缩放变化；a=scale * 1000。
export const UI_EVENT_SCALE_FACTOR = 8;
/// 平台请求重绘。
export const UI_EVENT_REDRAW = 9;

/// 箭头游标。
export const CURSOR_DEFAULT = 0;
/// 手型游标（可点击元素）。
export const CURSOR_HAND = 1;
/// 文本 I 型游标。
export const CURSOR_TEXT = 2;
/// 等待游标。
export const CURSOR_WAIT = 3;

/// 一条已弹出的平台事件。不同事件使用 a 到 d 字段传递基础数据（无分配）。
export struct UiEvent {
    kind: int;
    a: int;
    b: int;
    c: int;
    d: int;
}

/// 保留绘制命令的 Canvas。坐标使用逻辑像素，运行时自动按当前 DPI 缩放；
/// 全部方法返回 this 支持链式绘制。命令在 present() 时一次性光栅化并清空。
export class Canvas {
    private handle: ptr<void>;
    internal constructor(handle: ptr<void>) {
        this.handle = handle;
    }
    /// 清空当前帧命令并设置背景颜色（0xAARRGGBB）。
    public function clear(color: int): Canvas {
        sw_ui_canvas_clear(this.handle, color);
        return this;
    }
    /// 添加一个实心矩形。
    public function fill_rect(x: float, y: float, w: float, h: float, color: int): Canvas {
        sw_ui_canvas_fill_rect(this.handle, x, y, w, h, color);
        return this;
    }
    /// 添加一个圆角实心矩形（r 为圆角半径）。
    public function fill_round_rect(x: float, y: float, w: float, h: float, r: float, color: int): Canvas {
        sw_ui_canvas_fill_round_rect(this.handle, x, y, w, h, r, color);
        return this;
    }
    /// 添加一个圆角描边矩形（stroke_w 为线宽）。
    public function stroke_round_rect(x: float, y: float, w: float, h: float, r: float, stroke_w: float, color: int): Canvas {
        sw_ui_canvas_stroke_round_rect(this.handle, x, y, w, h, r, stroke_w, color);
        return this;
    }
    /// 添加一个实心圆。
    public function fill_circle(cx: float, cy: float, r: float, color: int): Canvas {
        sw_ui_canvas_fill_circle(this.handle, cx, cy, r, color);
        return this;
    }
    /// 添加一个线性渐变矩形（端点坐标与矩形同坐标系，逻辑像素）。
    public function fill_linear_gradient(x: float, y: float, w: float, h: float,
                                         x0: float, y0: float, x1: float, y1: float,
                                         c0: int, c1: int): Canvas {
        sw_ui_canvas_fill_linear_gradient(this.handle, x, y, w, h, x0, y0, x1, y1, c0, c1);
        return this;
    }
    /// 绘制文本（左上角对齐，size 为逻辑像素高度）。
    public function draw_text(x: float, y: float, size: float, text: string, color: int): Canvas {
        sw_ui_canvas_draw_text(this.handle, x, y, size, text, color);
        return this;
    }
    /// 测量文本宽度（逻辑像素）。
    public function text_width(size: float, text: string): float {
        return sw_ui_canvas_text_width(this.handle, size, text);
    }
}

/// 跨平台窗口与事件宿主。一个 Application 对应一个原生窗口；
/// 事件循环由主线程驱动（pump），后端线程永不进入 Sw 代码。
export class Application {
    private handle: ptr<void>;
    /// 创建可见窗口。宽高为逻辑像素；失败时 is_open() 为 false、
    /// last_error() 给出原因。
    public constructor(title: string, width: int, height: int) {
        this.handle = sw_ui_create(title, width, height);
    }
    /// 等待并收集平台事件，最长阻塞 timeout_ms 毫秒；窗口仍开时返回 true。
    public function pump(timeout_ms: int): bool {
        sw_ui_pump(this.handle, timeout_ms);
        return sw_ui_is_open(this.handle) != 0;
    }
    /// 窗口尚未收到关闭请求时返回 true。
    public function is_open(): bool {
        return sw_ui_is_open(this.handle) != 0;
    }
    /// 主动结束窗口循环并隐藏窗口。
    public function close(): void {
        sw_ui_close(this.handle);
    }
    /// 取出一条事件；队列为空时 kind 为 UI_EVENT_NONE。
    public function poll(): UiEvent {
        const kind = sw_ui_event_poll(this.handle);
        return {
            kind,
            a: sw_ui_event_a(this.handle),
            b: sw_ui_event_b(this.handle),
            c: sw_ui_event_c(this.handle),
            d: sw_ui_event_d(this.handle)
        };
    }
    /// 设置窗口标题；可在事件循环中调用。
    public function set_title(title: string): Application {
        sw_ui_set_title(this.handle, title);
        return this;
    }
    /// 请求新的逻辑尺寸；实际结果通过 UI_EVENT_RESIZED 观察。
    public function set_size(width: int, height: int): Application {
        sw_ui_set_size(this.handle, width, height);
        return this;
    }
    /// 请求平台重绘，通常由控件状态或动画更新后调用。
    public function request_redraw(): Application {
        sw_ui_request_redraw(this.handle);
        return this;
    }
    /// 设置窗口游标。
    public function set_cursor(cursor: int): Application {
        sw_ui_set_cursor(this.handle, cursor);
        return this;
    }
    /// 设置自定义标题栏拖动区域高度（逻辑像素）；区域内的按下可拖动窗口。
    /// frameless 窗口默认无系统标题栏，用本方法声明可拖动区域。
    public function set_title_bar(height: int): Application {
        sw_ui_set_title_bar(this.handle, height);
        return this;
    }
    /// 取绘制表面；绘制命令可链式调用，最后用 present() 提交。
    public function canvas(): Canvas {
        return new Canvas(this.handle);
    }
    /// 提交绘制命令到窗口；返回 false 表示渲染后端错误。
    public function present(): bool {
        return sw_ui_present(this.handle) == 0;
    }
    /// 当前 DPI 缩放 ×1000（如 125% 显示 → 1250）。
    public function scale(): int {
        return sw_ui_get_scale(this.handle);
    }
    /// 最近一次原生后端错误码；0 表示无错误，非 0 表示 UI 不可用。
    public function last_error(): int {
        return sw_ui_last_error(this.handle);
    }
    /// 释放窗口资源；必须在循环退出后且仅调用一次。
    public function destroy(): void {
        sw_ui_destroy(this.handle);
    }
}
