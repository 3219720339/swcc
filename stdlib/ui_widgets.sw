// std/ui_widgets - 基础控件 + 深浅主题（纯 Sw，构建于 std/ui 的 Canvas 之上）。
// 控件是数据驱动：draw() 绘制、hit() 命中测试，事件由宿主循环分发。
import { Canvas } from "std/ui";

/// 主题色板。
export struct Theme {
    /// 窗口背景渐变（起/止）。
    bg_top: int;
    bg_bottom: int;
    /// 面板底色 / 描边。
    panel: int;
    border: int;
    /// 正文 / 次要文字。
    text: int;
    text_muted: int;
    /// 主色 / 悬停色。
    accent: int;
    accent_hover: int;
    /// 标题栏底色 / 标题文字。
    title_bar: int;
    title_text: int;
}

/// 浅色主题。
export function theme_light(): Theme {
    return {
        bg_top: 0xFFECEFF1,
        bg_bottom: 0xFFCFD8DC,
        panel: 0xFFFFFFFF,
        border: 0xFFB0BEC5,
        text: 0xFF263238,
        text_muted: 0xFF78909C,
        accent: 0xFF1976D2,
        accent_hover: 0xFF1565C0,
        title_bar: 0xFF37474F,
        title_text: 0xFFFFFFFF
    };
}

/// 深色主题。
export function theme_dark(): Theme {
    return {
        bg_top: 0xFF263238,
        bg_bottom: 0xFF1A1A1A,
        panel: 0xFF37474F,
        border: 0xFF546E7A,
        text: 0xFFECEFF1,
        text_muted: 0xFF90A4AE,
        accent: 0xFF42A5F5,
        accent_hover: 0xFF1E88E5,
        title_bar: 0xFF0D1B2A,
        title_text: 0xFFFFFFFF
    };
}

/// 圆角面板容器。
export class Panel {
    public x: float;
    public y: float;
    public w: float;
    public h: float;
    public r: float;
    public constructor(x: float, y: float, w: float, h: float, r: float) {
        this.x = x;
        this.y = y;
        this.w = w;
        this.h = h;
        this.r = r;
    }
    public function draw(canvas: Canvas, theme: Theme): void {
        canvas
            .fill_round_rect(this.x, this.y, this.w, this.h, this.r, theme.panel)
            .stroke_round_rect(this.x, this.y, this.w, this.h, this.r, 1.0, theme.border);
    }
}

/// 文本标签。
export class Label {
    public x: float;
    public y: float;
    public size: float;
    public text: string;
    public color: int;
    public constructor(x: float, y: float, size: float, text: string, color: int) {
        this.x = x;
        this.y = y;
        this.size = size;
        this.text = text;
        this.color = color;
    }
    public function draw(canvas: Canvas): void {
        canvas.draw_text(this.x, this.y, this.size, this.text, this.color);
    }
}

/// 圆角按钮（悬停变色）。
export class Button {
    public x: float;
    public y: float;
    public w: float;
    public h: float;
    public label: string;
    private hovered: bool;
    public constructor(x: float, y: float, w: float, h: float, label: string) {
        this.x = x;
        this.y = y;
        this.w = w;
        this.h = h;
        this.label = label;
        this.hovered = false;
    }
    public function draw(canvas: Canvas, theme: Theme): void {
        const bg = this.hovered ? theme.accent_hover : theme.accent;
        canvas.fill_round_rect(this.x, this.y, this.w, this.h, 8.0, bg);
        const tw = canvas.text_width(16.0, this.label);
        canvas.draw_text(this.x + (this.w - tw) / 2.0, this.y + 10.0, 16.0, this.label, 0xFFFFFFFF);
    }
    /// 逻辑坐标命中测试。
    public function hit(px: float, py: float): bool {
        return px >= this.x && px <= this.x + this.w && py >= this.y && py <= this.y + this.h;
    }
    public function set_hovered(h: bool): void {
        this.hovered = h;
    }
}
