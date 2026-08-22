param(
    [string]$Header = "include\cribra.h",
    [string]$Library = "target\debug\cribra_capi.dll"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $Header)) {
    throw "missing generated header: $Header"
}

if (-not (Test-Path $Library)) {
    throw "missing native dynamic library: $Library"
}

$headerText = Get-Content -Raw $Header
$expected = [regex]::Matches(
    $headerText,
    '\b(cribra_[A-Za-z0-9_]+)\s*\('
) | ForEach-Object {
    $_.Groups[1].Value
} | Sort-Object -Unique

if ($expected.Count -eq 0) {
    throw "no cribra_* functions found in generated header"
}

$dump = & dumpbin /nologo /exports $Library
if ($LASTEXITCODE -ne 0) {
    throw "dumpbin failed"
}

$actual = $dump | ForEach-Object {
    if ($_ -match '\b(cribra_[A-Za-z0-9_]+)\s*$') {
        $Matches[1]
    }
} | Sort-Object -Unique

if ($actual.Count -eq 0) {
    throw "no cribra_* functions exported by native dynamic library"
}

$missing = Compare-Object -ReferenceObject $expected -DifferenceObject $actual |
    Where-Object { $_.SideIndicator -eq '<=' }

$unexpected = Compare-Object -ReferenceObject $expected -DifferenceObject $actual |
    Where-Object { $_.SideIndicator -eq '=>' }

if ($missing -or $unexpected) {
    if ($missing) {
        Write-Error "symbols declared in header but missing from DLL:"
        $missing | ForEach-Object { Write-Error "  $($_.InputObject)" }
    }

    if ($unexpected) {
        Write-Error "cribra_* symbols exported by DLL but absent from header:"
        $unexpected | ForEach-Object { Write-Error "  $($_.InputObject)" }
    }

    throw "native C ABI symbol set differs from generated header"
}

Write-Host "cribra-capi symbols: ok ($($actual.Count) exports)"
