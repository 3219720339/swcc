// probe-ui.sw — UI P1 探针：建窗、泵事件、打印事件、自动关闭。
// 注册于 run-examples.py（期望退出码 0）；无 UI 平台（stub）时优雅跳过。
import { println } from "std/io";
import {
    Application,
    UI_EVENT_NONE,
    UI_EVENT_CLOSE_REQUESTED,
    UI_EVENT_RESIZED,
    UI_EVENT_KEY,
    UI_EVENT_CURSOR,
    UI_EVENT_MOUSE_BUTTON,
    UI_EVENT_MOUSE_WHEEL,
    UI_EVENT_FOCUS,
    UI_EVENT_SCALE_FACTOR,
    CURSOR_HAND
} from "std/ui";

function event_name(kind: int): string {
    if (kind == UI_EVENT_NONE) return "NONE";
    if (kind == UI_EVENT_CLOSE_REQUESTED) return "CLOSE";
    if (kind == UI_EVENT_RESIZED) return "RESIZED";
    if (kind == UI_EVENT_KEY) return "KEY";
    if (kind == UI_EVENT_CURSOR) return "CURSOR";
    if (kind == UI_EVENT_MOUSE_BUTTON) return "MOUSE_BUTTON";
    if (kind == UI_EVENT_MOUSE_WHEEL) return "WHEEL";
    if (kind == UI_EVENT_FOCUS) return "FOCUS";
    if (kind == UI_EVENT_SCALE_FACTOR) return "SCALE";
    return "?";
}

function main(): int {
    const app = new Application("swc UI probe", 480, 320);
    if (app.last_error() != 0) {
        println("UI 不可用（错误码 " + app.last_error() + "），跳过");
        return 0;
    }
    // 链式调用：set_title → set_cursor
    app.set_title("swc UI probe (P1)").set_cursor(CURSOR_HAND);
    println("scale=" + app.scale());
    let events = 0;
    let printed = 0;
    let frames = 0;
    while (app.pump(16) && frames < 150) {
        frames = frames + 1;
        while (true) {
            const ev = app.poll();
            if (ev.kind == UI_EVENT_NONE) break;
            events = events + 1;
            if (ev.kind != UI_EVENT_CURSOR) {
                println(event_name(ev.kind) + " a=" + ev.a + " b=" + ev.b + " c=" + ev.c);
                printed = printed + 1;
            }
        }
    }
    app.destroy();
    println("UI 探针完成：frames=" + frames + " events=" + events + " printed=" + printed);
    return 0;
}
