import { println } from "std/io";
import { cli_parse, cli_has, cli_get, cli_get_int, cli_values, cli_help } from "std/cli";
import { temp_file_create, atomic_write, read_all, stat, FILE_REGULAR, path_canonicalize, remove } from "std/fs";
import { json_object_new, json_string_new, json_object_set, json_write_file_pretty, json_read_file, json_string, json_object_get } from "std/json";
import { toml_parse, toml_write_file, toml_read_file, toml_get } from "std/toml";
import { bytes_hex_encode, bytes_hex_decode, bytes_read_u16_le, bytes_read_u32_be, bytes_u16_le, bytes_u32_be } from "std/bytes";
import { random_bytes, constant_time_equal, secure_token } from "std/crypto";
import { config_parse_dotenv, config_apply_env, config_get } from "std/config";
import { map_new, map_set } from "std/map";
import { setenv } from "std/os";
import { log_json, LOG_INFO } from "std/log";

function check(condition: bool, label: string): int {
    if (condition) { println(`[ok] ${label}`); return 1; }
    println(`[FAIL] ${label}`);
    return 0;
}

function main(args: string[]): int {
    let passed = 1;

    const cli = cli_parse(args);
    passed = passed & check(cli.command == "build", "cli subcommand");
    passed = passed & check(cli.positionals.length == 1 && cli.positionals[0] == "input.sw", "cli positional");
    passed = passed & check(cli_has(cli, "-v") && cli_has(cli, "-d"), "cli short flags");
    passed = passed & check((cli_get(cli, "--port") ?? "") == "9090" && cli_get_int(cli, "--port", 0) == 9090, "cli option value");
    const tags = cli_values(cli, "--tag");
    passed = passed & check(tags.length == 2 && tags[0] == "one" && tags[1] == "two", "cli repeated values");
    passed = passed & check(cli_help("tool", "<command>", ["  -v  verbose"]).contains("Usage: tool"), "cli help");

    const path = temp_file_create("swc");
    passed = passed & check(path != "", "secure temp file");
    passed = passed & check(atomic_write(path, "first") == 5 && atomic_write(path, "second") == 6 && read_all(path) == "second", "atomic write text");
    const info = stat(path);
    passed = passed & check(info.exists && info.kind == FILE_REGULAR && info.size == 6 && info.modified > 0, "file stat");
    passed = passed & check(path_canonicalize(path).length >= path.length, "canonical path");

    const doc = json_object_new();
    json_object_set(doc, "name", json_string_new("sw"));
    passed = passed & check(json_write_file_pretty(path, doc) > 0 && json_string(json_object_get(json_read_file(path), "name")) == "sw", "json file roundtrip");
    const toml = toml_parse("server.port = \"8080\"\n");
    passed = passed & check(toml_write_file(path, toml) > 0 && (toml_get(toml_read_file(path), "server.port") ?? "") == "8080", "toml file roundtrip");

    const bytes: u8[] = [0 as u8, 15 as u8, 255 as u8, 1 as u8];
    passed = passed & check(bytes_hex_encode(bytes) == "000fff01" && bytes_hex_decode("000fff01").length == 4, "bytes hex");
    passed = passed & check(bytes_read_u16_le(bytes_u16_le(0x1234), 0) == 0x1234 && bytes_read_u32_be(bytes_u32_be(0x1020304), 0) == 0x1020304, "bytes endian");
    const random = random_bytes(16);
    const different: u8[] = [1 as u8];
    passed = passed & check(random.length == 16 && constant_time_equal(random, random) && !constant_time_equal(random, different), "crypto random and compare");
    passed = passed & check(secure_token(12).length >= 16, "crypto token");

    const config = config_parse_dotenv("PORT=8080\nNAME=sw\n");
    setenv("APP_PORT", "9090");
    const merged = config_apply_env(config, "APP_");
    passed = passed & check(config_get(merged, "PORT", "") == "9090" && config_get(merged, "NAME", "") == "sw", "config dotenv and env");
    const fields = map_new();
    map_set(fields, "probe", "batch14");
    log_json(LOG_INFO, "structured log", fields);

    remove(path);
    println(`final=${passed == 1 ? "PASS" : "FAIL"}`);
    return passed == 1 ? 0 : 1;
}
