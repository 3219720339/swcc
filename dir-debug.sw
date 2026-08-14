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
    const entries = list_dir("dir-demo");
    trace(`list=${entries.length}`);
    let names = "";
    let i = 0;
    while (i < entries.length) {
        names = `${names}${entries[i]} `;
        i = i + 1;
    }
    trace(`names=${names}`);
    const r4 = rename("dir-demo/a.txt", "dir-demo/renamed.txt");
    trace(`rename=${r4}`);
    const r5 = copy_file("dir-demo/b.txt", "dir-demo/copy.txt");
    trace(`copy=${r5}`);
    const b1 = path_basename("dir-demo/renamed.txt");
    const d1 = path_dirname("dir-demo/renamed.txt");
    const e1 = path_ext("dir-demo/renamed.txt");
    trace(`path=${b1}|${d1}|${e1}`);
    const r6 = exists("dir-demo/copy.txt");
    trace(`exists=${r6}`);
    const r7 = is_dir("dir-demo");
    trace(`isdir=${r7}`);
    const r8 = remove("dir-demo/copy.txt");
    trace(`remove=${r8}`);
    const r9 = exists("dir-demo/copy.txt");
    trace(`removed-exists=${r9}`);
    trace("done");
    return 0;
}
