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
export extern c function sw_ui_last_error(handle: ptr<void>): int;

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
