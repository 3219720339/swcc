// probe-ui.sw — UI P2 探针：建窗、绘制（渐变/圆角/文字）、泵事件、自动关闭。
// 注册于 run-examples.py（期望退出码 0）；无 UI 平台（stub）时优雅跳过。
import { println } from "std/io";
import * as ui from "std/ui";

function main(): int {
    const app = new ui.Application("swc UI probe", 420, 300);
    if (app.last_error() != 0) {
        println("UI 不可用（错误码 " + app.last_error() + "），跳过");
        return 0;
    }
    // 链式：标题栏拖动区 + 手型游标
    app.set_title("swc UI probe (P2)").set_title_bar(36).set_cursor(ui.CURSOR_HAND);
    println("scale=" + app.scale());
    let events = 0;
    let frames = 0;
    while (app.pump(16) && frames < 120) {
        frames = frames + 1;
        while (true) {
            const ev = app.poll();
            if (ev.kind == ui.UI_EVENT_NONE) break;
            events = events + 1;
            if (ev.kind != ui.UI_EVENT_CURSOR) {
                println(ev.kind + " a=" + ev.a + " b=" + ev.b);
            }
        }
        // 每帧绘制：渐变背景 + 圆角面板 + 标题文字 + 圆形 + 按钮
        const canvas = app.canvas();
        canvas
            .fill_linear_gradient(0.0, 0.0, 420.0, 300.0, 0.0, 0.0, 0.0, 300.0, 0xFFE8F0FE, 0xFFBBDEFB)
            .fill_round_rect(40.0, 60.0, 340.0, 180.0, 12.0, 0xFFFFFFFF)
            .fill_round_rect(60.0, 80.0, 200.0, 120.0, 8.0, 0xFF4A90D9)
            .fill_circle(300.0, 120.0, 36.0, 0xFFFFA726)
            .draw_text(80.0, 110.0, 20.0, "你好 Sw UI", 0xFFFFFFFF)
            .draw_text(60.0, 220.0, 14.0, "frame=" + frames + "  events=" + events, 0xFF455A64);
        app.present();
    }
    app.destroy();
    println("UI 探针完成：frames=" + frames + " events=" + events);
    return 0;
}
