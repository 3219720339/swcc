// std/thread - 受 GC 保护的原生线程。
//
// 当前线程入口只接受 `() => int` 闭包。捕获环境由运行时根保护到任务结束，
// 未捕获异常会转为 FAILED 状态，不会跨线程传播 longjmp。
// 块闭包内含循环的控制流降级仍在完善，首版线程入口请把循环放在普通函数中调用。
export const THREAD_RUNNING = 1;
export const THREAD_COMPLETED = 2;
export const THREAD_FAILED = 3;
export const THREAD_CANCELLED = 4;
/// 创建线程。callback 必须是无参数、返回 int 的闭包；失败返回 -1。
export extern c function thread_spawn(callback: any): int;
/// 等待线程结束。0 完成，1 超时，-1 无效或已 detach。-1 表示无限等待。
export extern c function thread_join(thread: int, timeout_ms: int): int;
/// 分离线程。分离后不能 join，但仍可查询状态和结果。
export extern c function thread_detach(thread: int): int;
/// 当前状态：RUNNING、COMPLETED、FAILED、CANCELLED；无效线程为 -1。
export extern c function thread_state(thread: int): int;
/// 线程正常完成后的 int 结果；其他状态返回 0。
export extern c function thread_result(thread: int): int;
/// 线程失败时的异常类型编号；其他状态返回 0。
export extern c function thread_exception_type(thread: int): int;
export function thread_join_forever(thread: int): int {
    return thread_join(thread, - 1);
}
