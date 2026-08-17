// std/thread - 受 GC 保护的原生线程。
//
// 当前线程入口只接受 `() => int` 闭包。捕获环境由运行时根保护到任务结束，
// 未捕获异常会转为 FAILED 状态，不会跨线程传播 longjmp。
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

/// 带参任务：`spawn_with(f, arg)` 等价于 `thread_spawn(() => f(arg))`。
/// arg 按闭包值捕获传入线程；返回线程 id。
export function spawn_with<A>(f: (A) => int, arg: A): int {
    return thread_spawn((): int => f(arg));
}

/// 带参 + 结果回传：任务把 `f(arg)` 的结果写入 out[0]（out 是引用类型，
/// 任务内写入对调用方可见），返回线程 id。适合 string/class/struct 等
/// 复杂结果（int 结果可直接用 thread_result）。
export function spawn_result<A, R>(f: (A) => R, arg: A, out: R[]): int {
    return thread_spawn((): int => {
        out[0] = f(arg);
        return 0;
    });
}
