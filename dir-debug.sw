import { println } from "std/io";
import {
    list_dir,
    is_dir,
    mkdir,
    remove,
    rename,
    copy_file,
    write_all,
    read_all,
    exists,
    path_basename,
    path_dirname,
    path_ext,
} from "std/fs";

function trace(step: string): void {
    const prev = read_all("dir-trace.txt");
    write_all("dir-trace.txt", `${prev}${step}|`);
    println(`step: ${step}`);
}

function main(): int {
    trace("start");
    const r1 = mkdir("dir-demo");
    trace(`mkdir=${r1}`);
    const r2 = write_all("dir-demo/a.txt", "hello");
    trace(`write-a=${r2}`);
    const r3 = write_all("dir-demo/b.txt", "world");
    trace(`write-b=${r3}`);
    const r4 = mkdir("dir-empty");
    trace(`mkdir-empty=${r4}`);
    const i1 = is_dir("dir-demo");
    trace(`isdir-demo=${i1}`);
    const e0 = list_dir("dir-empty");
    trace(`list-empty=${e0.length}`);
    const entries = list_dir("dir-demo");
    trace(`list=${entries.length}`);
    let names = "";
    let i = 0;
    while (i < entries.length) {
        names = `${names}${entries[i]} `;
        i = i + 1;
    }
    trace(`names=${names}`);
    trace("done");
    return 0;
}
