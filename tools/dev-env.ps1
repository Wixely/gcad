# Sets up a Windows shell for building gcad without relying on vswhere or VsDevCmd.
# Usage:  . .\tools\dev-env.ps1 ; cargo build --features game-assets
# It finds the newest MSVC toolset and Windows SDK, puts them and ~/.cargo/bin on PATH, and
# looks for libclang (needed by the game-assets feature's xatlas bindings) when
# LIBCLANG_PATH is not already set.

$ErrorActionPreference = "Stop"

$vsRoots = @("C:\Program Files\Microsoft Visual Studio", "${env:ProgramFiles(x86)}\Microsoft Visual Studio")
$msvc = Get-ChildItem -Path $vsRoots -Directory -Recurse -Depth 4 -Filter MSVC -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -like "*\VC\Tools\MSVC" } |
    ForEach-Object { Get-ChildItem $_.FullName -Directory } |
    Where-Object { Test-Path "$($_.FullName)\bin\Hostx64\x64\link.exe" } |
    Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
if (-not $msvc) { throw "No MSVC toolset with Hostx64\x64\link.exe found" }

$kits = "${env:ProgramFiles(x86)}\Windows Kits\10"
$sdk = Get-ChildItem "$kits\Lib" -Directory |
    Where-Object { Test-Path "$($_.FullName)\um\x64\kernel32.lib" } |
    Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
if (-not $sdk) { throw "No Windows 10/11 SDK with um\x64 libs found under $kits\Lib" }

$v = $sdk.Name
$m = $msvc.FullName

$env:PATH = "$m\bin\Hostx64\x64;$kits\bin\$v\x64;$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:LIB = "$m\lib\x64;$kits\Lib\$v\ucrt\x64;$kits\Lib\$v\um\x64"
$env:INCLUDE = "$m\include;$kits\Include\$v\ucrt;$kits\Include\$v\um;$kits\Include\$v\shared"

if (-not $env:LIBCLANG_PATH) {
    $clang = @("C:\Program Files\LLVM\bin", "$m\..\..\..\Llvm\x64\bin", "$env:USERPROFILE\scoop\apps\llvm\current\bin") |
        Where-Object { Test-Path (Join-Path $_ "libclang.dll") } | Select-Object -First 1
    if ($clang) { $env:LIBCLANG_PATH = (Resolve-Path $clang).Path }
}
$libclang = if ($env:LIBCLANG_PATH) { $env:LIBCLANG_PATH } else { "not found (install LLVM or set LIBCLANG_PATH)" }

Write-Host "MSVC $($msvc.Name), Windows SDK $v, libclang: $libclang"
