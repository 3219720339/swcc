# Build a Windows x64 self-contained SDK (drop-in replacement for make-sdk.py
# on machines without Python).
# Usage: powershell -ExecutionPolicy Bypass -File tools/make-sdk.ps1
#        [-Toolchain D:\llvm-mingw-20260616-ucrt-x86_64] [-OutDir dist]
# Output: <OutDir>/swc-windows-x64-<version>.zip (version from root Cargo.toml)
# Never deletes existing files; re-running with the same version overwrites
# the zip in place.
param(
    [string]$Toolchain = "D:\llvm-mingw-20260616-ucrt-x86_64",
    [string]$OutDir = "dist"
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot

$match = Select-String -Path (Join-Path $Root "Cargo.toml") -Pattern '^\s*version\s*=\s*"([^"]+)"' | Select-Object -First 1
if (-not $match) {
    throw "Cannot read version from Cargo.toml"
}
$Version = $match.Matches[0].Groups[1].Value
Write-Host "Packaging swc $Version ..."

cargo build --release -p swc | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "cargo build failed"
}

$clang = Join-Path $Toolchain "bin\clang.exe"
$lld = Join-Path $Toolchain "bin\ld.lld.exe"
$mingwLib = Join-Path $Toolchain "x86_64-w64-mingw32\lib"
$builtins = Join-Path $Toolchain "lib\clang\22\lib\windows\libclang_rt.builtins-x86_64.a"
foreach ($path in @($clang, $lld, $mingwLib, $builtins)) {
    if (-not (Test-Path $path)) {
        throw "Toolchain missing: $path"
    }
}

$sdk = Join-Path $Root (Join-Path $OutDir "swc-windows-x64-$Version")
foreach ($sub in @("bin", "lib", "stdlib")) {
    New-Item -ItemType Directory -Force (Join-Path $sdk $sub) | Out-Null
}

Copy-Item (Join-Path $Root "target\release\swc.exe") (Join-Path $sdk "swc.exe") -Force
Copy-Item $lld (Join-Path $sdk "bin\ld.lld.exe") -Force
foreach ($dll in @("libLLVM-22.dll", "libc++.dll", "libunwind.dll")) {
    $source = Join-Path $Toolchain "bin\$dll"
    if (Test-Path $source) {
        Copy-Item $source (Join-Path $sdk "bin\$dll") -Force
    }
}
foreach ($name in @("libucrt.a", "libucrtbase.a", "libkernel32.a", "libshell32.a")) {
    Copy-Item (Join-Path $mingwLib $name) (Join-Path $sdk "lib\$name") -Force
}
Copy-Item $builtins (Join-Path $sdk "lib\libclang_rt.builtins-x86_64.a") -Force

$target = "x86_64-w64-windows-gnu"
$runtimeDir = Join-Path $Root "runtime"
& $clang -target $target -O2 -c (Join-Path $runtimeDir "runtime.c") -o (Join-Path $sdk "lib\runtime.obj")
if ($LASTEXITCODE -ne 0) { throw "compile runtime.c failed" }
& $clang -target $target -c (Join-Path $runtimeDir "runtime_x64.S") -o (Join-Path $sdk "lib\runtime_asm.obj")
if ($LASTEXITCODE -ne 0) { throw "compile runtime_x64.S failed" }
& $clang -target $target -c (Join-Path $runtimeDir "startup.s") -o (Join-Path $sdk "lib\startup.obj")
if ($LASTEXITCODE -ne 0) { throw "compile startup.s failed" }

Copy-Item (Join-Path $Root "stdlib\*.sw") (Join-Path $sdk "stdlib") -Force
[System.IO.File]::WriteAllText(
    (Join-Path $sdk "version.txt"),
    "swc $Version`r`n",
    (New-Object System.Text.UTF8Encoding($false))
)

$archive = Join-Path $Root (Join-Path $OutDir "swc-windows-x64-$Version.zip")
Compress-Archive -Path (Join-Path $sdk "*") -DestinationPath $archive -CompressionLevel Optimal -Force
Write-Host "SDK ready: $archive"
