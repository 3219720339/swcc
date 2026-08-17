// demo-ui.sw — Sw 跨平台 UI 演示：frameless 窗口、自定义标题栏、深浅主题、
// 控件（Panel/Label/Button）、渐变、悬停反馈。窗口常驻，点右上角 X 或按
// Esc 关闭。运行：swc run examples/demo-ui.sw
import * as ui from "std/ui";
import { println } from "std/io";
import { Button, Label, Panel, theme_light, theme_dark } from "std/ui_widgets";

function main(): int {
    const app = new ui.Application("Sw UI Demo", 560, 400);
    if (app.last_error() != 0) {
        println("UI 不可用（错误码 " + app.last_error() + "），跳过");
        return 0;
    }
    app.set_title_bar(40);
    const scale = app.scale();

    // 控件（逻辑坐标）
    const title = new Label(20.0, 12.0, 17.0, "Sw UI Demo", 0xFFFFFFFF);
    const subtitle = new Label(150.0, 15.0, 12.0, "cross-platform · minimal deps", 0xFFB0BEC5);
    const panel = new Panel(40.0, 80.0, 480.0, 200.0, 12.0);
    const body = new Label(64.0, 108.0, 15.0, "这是一个 Sw 编写的跨平台桌面 UI 演示。", 0xFF000000);
    const body2 = new Label(64.0, 136.0, 13.0, "CPU 光栅化 · 无第三方依赖 · DPI 自适应", 0xFF000000);
    const status = new Label(64.0, 244.0, 12.0, "", 0xFF000000);
    const hint = new Label(64.0, 330.0, 12.0, "点击「切换主题」换肤 · 点右上角 X 或按 Esc 退出", 0xFF000000);
    const btn_theme = new Button(80.0, 300.0, 150.0, 40.0, "切换主题");
    const btn_close = new Button(340.0, 300.0, 140.0, 40.0, "关闭窗口");

    let dark = false;
    let clicked = 0;
    let frames = 0;
    let mouse_x = 0.0;
    let mouse_y = 0.0;

    while (app.pump(16)) {
        frames = frames + 1;
        // —— 事件分发（数据驱动）——
        while (true) {
            const ev = app.poll();
            if (ev.kind == ui.UI_EVENT_NONE) break;
            if (ev.kind == ui.UI_EVENT_CURSOR) {
                // 事件坐标为物理像素，转逻辑像素。
                mouse_x = ev.a as float * 1000.0 / (scale as float);
                mouse_y = ev.b as float * 1000.0 / (scale as float);
            } else if (ev.kind == ui.UI_EVENT_MOUSE_BUTTON && ev.a == 1 && ev.b == 1) {
                // 右上角关闭按钮（逻辑坐标）
                if (mouse_x >= 524.0 && mouse_x <= 554.0 && mouse_y >= 8.0 && mouse_y <= 32.0) {
                    app.close();
                } else if (btn_theme.hit(mouse_x, mouse_y)) {
                    dark = !dark;
                    clicked = clicked + 1;
                } else if (btn_close.hit(mouse_x, mouse_y)) {
                    app.close();
                }
            } else if (ev.kind == ui.UI_EVENT_KEY && ev.a == 1 && ev.b == 1) {
                app.close(); // Esc
            }
        }
        // 悬停状态
        btn_theme.set_hovered(btn_theme.hit(mouse_x, mouse_y));
        btn_close.set_hovered(btn_close.hit(mouse_x, mouse_y));

        // —— 绘制 ——
        const theme = dark ? theme_dark() : theme_light();
        const canvas = app.canvas();
        body.color = theme.text;
        body2.color = theme.text_muted;
        status.text = "frames=" + frames + "  clicked=" + clicked + "  scale=" + scale + "  theme=" + (dark ? "dark" : "light");
        status.color = theme.text_muted;
        canvas
            .fill_linear_gradient(0.0, 0.0, 560.0, 400.0, 0.0, 0.0, 0.0, 400.0,
                                  theme.bg_top, theme.bg_bottom)
            // 标题栏（可拖动区域已由 set_title_bar 声明）
            .fill_round_rect(0.0, 0.0, 560.0, 40.0, 0.0, theme.title_bar);
        title.draw(canvas);
        subtitle.draw(canvas);
        // 右上角关闭按钮
        canvas
            .fill_round_rect(524.0, 8.0, 30.0, 24.0, 6.0, 0xFFE53935)
            .draw_text(534.0, 12.0, 14.0, "×", 0xFFFFFFFF);
        panel.draw(canvas, theme);
        body.draw(canvas);
        body2.draw(canvas);
        status.draw(canvas);
        btn_theme.draw(canvas, theme);
        btn_close.draw(canvas, theme);
        hint.draw(canvas);
        app.present();
    }
    app.destroy();
    println("demo 退出：frames=" + frames + " clicked=" + clicked);
    return 0;
}
