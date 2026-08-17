// 跨模块字符串全局定义模块（被 probe-string-global.sw 引用）。
export const APP_NAME = "swc-app";
export let version = "v1.0";

export function get_app(): string {
    return APP_NAME;
}

export function get_version(): string {
    return version;
}
