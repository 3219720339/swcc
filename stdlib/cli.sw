// std/cli - 命令行参数解析。CliArgs 的 options/values 为 string map 句柄。

import { map_new, map_get, map_set, map_has } from "std/map";
import { starts_with, index_of, substring, split, join, parse_int_or } from "std/string";

export struct CliArgs {
    command: string;
    positionals: string[];
    options: ptr<void>;
    values: ptr<void>;
}

function add_value(values: ptr<void>, key: string, value: string): void {
    if (!map_has(values, key)) {
        map_set(values, key, value);
        return;
    }
    const old = map_get(values, key) ?? "";
    map_set(values, key, old + "\u{1f}" + value);
}

/// 解析 main(args)；第一个非选项参数为子命令，后续为 positionals。
/// 支持 --name=value、--name value、-abc 短 flag 组合和 -- 之后的原样参数。
export function cli_parse(args: string[]): CliArgs {
    const options = map_new();
    const values = map_new();
    const positionals: string[] = [];
    let command = "";
    let positional_only = false;
    let i = args.length > 0 ? 1 : 0;
    while (i < args.length) {
        const arg = args[i];
        if (!positional_only && arg == "--") {
            positional_only = true;
            i++;
            continue;
        }
        if (!positional_only && starts_with(arg, "--") && arg.length > 2) {
            const eq = index_of(arg, "=");
            if (eq >= 0) {
                const key = substring(arg, 0, eq);
                const value = substring(arg, eq + 1, arg.length - eq - 1);
                map_set(options, key, value);
                add_value(values, key, value);
            } else if (i + 1 < args.length && !starts_with(args[i + 1], "-")) {
                i++;
                map_set(options, arg, args[i]);
                add_value(values, arg, args[i]);
            } else {
                map_set(options, arg, "true");
                add_value(values, arg, "true");
            }
        } else if (!positional_only && starts_with(arg, "-") && arg.length > 1) {
            const short_flags = substring(arg, 1, arg.length - 1);
            for (const flag of short_flags.chars()) {
                const key = "-" + flag;
                map_set(options, key, "true");
                add_value(values, key, "true");
            }
        } else if (command == "") {
            command = arg;
        } else {
            positionals.push(arg);
        }
        i++;
    }
    return { command, positionals, options, values };
}

export function cli_has(cli: CliArgs, name: string): bool {
    return map_get(cli.options, name) != null;
}

export function cli_get(cli: CliArgs, name: string): string? {
    return map_get(cli.options, name);
}

export function cli_get_or(cli: CliArgs, name: string, fallback: string): string {
    return cli_get(cli, name) ?? fallback;
}

export function cli_get_int(cli: CliArgs, name: string, fallback: int): int {
    return parse_int_or(cli_get(cli, name) ?? "", fallback);
}

/// 读取同一参数的全部值（重复 --tag a --tag b）。
export function cli_values(cli: CliArgs, name: string): string[] {
    const raw = map_get(cli.values, name) ?? "";
    return raw == "" ? [] : split(raw, "\u{1f}");
}

/// 长短参数二选一，例如 cli_get_any(cli, ["--verbose", "-v"])。
export function cli_get_any(cli: CliArgs, names: string[]): string? {
    for (const name of names) {
        const value = cli_get(cli, name);
        if (value != null) {
            return value;
        }
    }
    return null;
}

/// 生成固定格式帮助文本；options 每行自行描述，如 "  -v, --verbose  详细输出"。
export function cli_help(program: string, usage: string, options: string[]): string {
    return `Usage: ${program} ${usage}\n\nOptions:\n${join(options, "\n")}`;
}
