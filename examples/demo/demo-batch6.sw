import { println } from "std/io";
import {
    last_index_of_string,
    last_index_of_int,
    min_index_int,
    max_index_int,
    zip_strings,
    取最后出现位置文本,
    取最小值位置,
    数组配对,
} from "std/array";
import { toml_parse, toml_get, 解析TOML, 取TOML项 } from "std/toml";
import { slugify, 转网址别名 } from "std/string";

function main(): int {
    // 数组补充
    println(last_index_of_string(["a", "b", "a", "c"], "a"));
    println(last_index_of_int([1, 2, 1, 3], 1));
    println(min_index_int([5, 2, 8, 1]));
    println(max_index_int([5, 2, 8, 1]));
    println(取最后出现位置文本(["x", "y", "x"], "x"));
    println(取最小值位置([3, 1, 2]));
    const z = zip_strings(["a", "b", "c"], ["1", "2"]);
    println(z.length);
    println(z[0][0] + ":" + z[0][1]);
    println(z[1][0] + ":" + z[1][1]);
    const z2 = 数组配对(["x"], ["y"]);
    println(z2[0][0] + ":" + z2[0][1]);

    // TOML
    const cfg = toml_parse("[server]\nport = 8080\nhost = \"127.0.0.1\"\nenabled = true\n");
    println(toml_get(cfg, "server.port") ?? "(无)");
    println(toml_get(cfg, "server.host") ?? "(无)");
    println(toml_get(cfg, "server.enabled") ?? "(无)");
    const cfg2 = 解析TOML("[a]\nb = \"hello\"\n");
    println(取TOML项(cfg2, "a.b") ?? "(无)");

    // slugify
    println(slugify("Hello World!"));
    println(slugify("  Multiple   Spaces  "));
    println(slugify("Sw语言 Test"));
    println(转网址别名("Hello-World"));
    return 0;
}
