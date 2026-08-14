import { println } from "std/io";
import {
    base64_encode,
    base64_decode,
    hex_encode,
    hex_decode,
    url_encode,
    url_decode,
    base64url_encode,
    base64url_decode,
    html_escape,
} from "std/encoding";

function main(): int {
    const text = "hello 你好";
    const b64 = base64_encode(text);
    println(`base64_encode=${b64} decode=${base64_decode(b64)}`);
    const hex = hex_encode(text);
    println(`hex_encode=${hex} decode=${hex_decode(hex)}`);
    const q = url_encode("a b&c=中文");
    println(`url_encode=${q} decode=${url_decode(q)}`);
    const b64u = base64url_encode(text);
    println(`base64url_encode=${b64u} decode=${base64url_decode(b64u)}`);
    println(`html_escape=${html_escape(`<a href="x">'&'</a>`)}`);
    return 0;
}
