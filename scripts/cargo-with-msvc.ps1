# Run Cargo with the MSVC and Windows SDK library paths required by link.exe.
$ErrorActionPreference = "Stop"

$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) {
    throw "vswhere.exe not found; install Visual Studio Build Tools with the Desktop development with C++ workload"
}

$vsRoot = & $vswhere -latest -products * -property installationPath
if (-not $vsRoot) {
    throw "No Visual Studio installation was found"
}

$msvcVersionFile = Join-Path $vsRoot "VC\Auxiliary\Build\Microsoft.VCToolsVersion.default.txt"
$msvcVersion = if (Test-Path $msvcVersionFile) {
    (Get-Content $msvcVersionFile -Raw).Trim()
} else {
    Get-ChildItem (Join-Path $vsRoot "VC\Tools\MSVC") -Directory |
        Sort-Object { [version]$_.Name } -Descending |
        Select-Object -First 1 -ExpandProperty Name
}
if (-not $msvcVersion) {
    throw "No MSVC toolset was found below $vsRoot"
}
$msvcRoot = Join-Path $vsRoot "VC\Tools\MSVC\$msvcVersion"

$sdkRoot = "${env:ProgramFiles(x86)}\Windows Kits\10"
$sdkVersion = Get-ChildItem (Join-Path $sdkRoot "Lib") -Directory |
    Where-Object { Test-Path (Join-Path $_.FullName "ucrt\x64\ucrt.lib") } |
    Sort-Object { [version]$_.Name } -Descending |
    Select-Object -First 1 -ExpandProperty Name
if (-not $sdkVersion) {
    throw "No Windows 10/11 SDK containing ucrt\x64\ucrt.lib was found"
}

$env:Path = "$(Join-Path $msvcRoot 'bin\HostX64\x64');$env:USERPROFILE\.cargo\bin;$env:Path"
$env:INCLUDE = @(
    (Join-Path $msvcRoot "include"),
    (Join-Path $sdkRoot "Include\$sdkVersion\ucrt"),
    (Join-Path $sdkRoot "Include\$sdkVersion\um"),
    (Join-Path $sdkRoot "Include\$sdkVersion\shared"),
    (Join-Path $sdkRoot "Include\$sdkVersion\winrt")
) -join ";"
$env:LIB = @(
    (Join-Path $msvcRoot "lib\x64"),
    (Join-Path $msvcRoot "lib\onecore\x64"),
    (Join-Path $sdkRoot "Lib\$sdkVersion\ucrt\x64"),
    (Join-Path $sdkRoot "Lib\$sdkVersion\um\x64")
) -join ";"

Set-Location (Join-Path $PSScriptRoot "..")
& cargo @args
exit $LASTEXITCODE
