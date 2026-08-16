import {
    println
}
from "std/io";
import {
    sleep_ms
}
from "std/time";
import {
    THREAD_COMPLETED,
    THREAD_FAILED,
    THREAD_CANCELLED,
    thread_spawn,
    thread_join,
    thread_detach,
    thread_state,
    thread_result,
    thread_exception_type
}
from "std/thread";
import {
    TASK_COMPLETED,
    TASK_FAILED,
    TASK_CANCELLED,
    task_spawn,
    task_poll,
    task_join,
    task_cancel,
    task_cancelled,
    task_state,
    task_result
}
from "std/task";
function check(condition: bool, label: string): int {
    if (condition) {
        println(`[ok] ${label}`);
        return 1;
    }
    println(`[FAIL] ${label}`);
    return 0;
}
async function await_captured(value: int): int {
    const task = task_spawn(() => value + 5);
    return await task;
}
function main(): int {
    let passed = 1;
    const captured = 6;
    const worker = thread_spawn(() => {
        let i = 0;
        while (i < 40000) {
            i++;
        }
        return i + captured;
    });
    let pressure = "gc";
    let pressure_i = 0;
    while (pressure_i < 16000) {
        pressure = pressure + "x";
        pressure_i++;
    }
    passed = passed & check(worker > 0 && thread_join(worker, - 1) == 0, "thread join after captured closure and gc");
    passed = passed & check(thread_state(worker) == THREAD_COMPLETED && thread_result(worker) == 40006, "thread captured loop result and state");
    const failed = thread_spawn(() => {
        throw "worker failure"; return 0;
    });
    passed = passed & check(thread_join(failed, - 1) == 0, "thread exception join");
    passed = passed & check(thread_state(failed) == THREAD_FAILED && thread_exception_type(failed) == 0, "thread exception is isolated");
    const task = task_spawn(() => 40 + 2);
    passed = passed & check(task > 0 && task_poll(task) >= 1 && task_join(task, - 1) == 0, "task spawn poll join");
    passed = passed & check(task_state(task) == TASK_COMPLETED && task_result(task) == 42, "task future result");
    passed = passed & check(await_captured(7) == 12, "async await task expression");
    const cancelled = task_spawn(() => task_cancelled());
    task_cancel(cancelled);
    passed = passed & check(task_join(cancelled, - 1) == 0 && task_state(cancelled) == TASK_CANCELLED, "cooperative task cancellation");
    const detached = thread_spawn(() => 9);
    passed = passed & check(thread_detach(detached) == 0, "thread detach");
    let spins = 0;
    while (thread_state(detached) == 1 && spins < 100) {
        sleep_ms(1);
        spins++;
    }
    passed = passed & check(thread_state(detached) == THREAD_COMPLETED || thread_state(detached) == THREAD_CANCELLED, "detached thread completes");
    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1?0: 1;
}
