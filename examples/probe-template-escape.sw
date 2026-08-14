import { println } from "std/io";

function main(): int {
    // \n 换行、\t 制表符（模板字符串内转义，与普通字符串一致）
    println(`line1\nline2\tindented`);
    // \u{...} 与 \xNN 转义生成汉字
    println(`中\u{6587}\xE4\xB8\xAD`);
    // 转义反引号与反斜杠
    println(`tick\`and\\slash`);
    // 转义与插值混用
    const name = "Sw";
    println(`name=${name}\nnext`);
    // 模板字符串可跨行，真实换行符原样保留
    const lines = `first
second`;
    println(lines);
    return 0;
}
