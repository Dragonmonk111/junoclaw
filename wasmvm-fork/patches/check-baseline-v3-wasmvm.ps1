# check-baseline-v3-wasmvm.ps1
#
# Day-3 of Track B: check whether the previously-dropped wasmvm-side
# wrappers (`wasmvm.api.rs.patch.dropped`, `wasmvm.lib.go.patch.dropped`)
# apply to a clean `wasmvm` v3.0.4 checkout. v3.0.4 has the BLS12-381 Go
# wrapper analogue our BN254 wrappers mirror, so most of the drift should
# be cosmetic (line shifts), not structural.
#
# Idempotent: first run clones wasmvm v3.0.4 into the build dir; subsequent
# runs reset to the tag and re-apply.

param(
    [string]$WasmvmTag = "v3.0.4",
    [string]$BuildDir  = (Join-Path $env:USERPROFILE "junoclaw-build")
)

$ErrorActionPreference = "Continue"

$JunoclawRoot = (Get-Item $PSScriptRoot).Parent.Parent.FullName
$WasmvmDir    = Join-Path $BuildDir "wasmvm"
$PatchDir     = Join-Path $JunoclawRoot "wasmvm-fork\patches"

# The two dropped patches we want to forward-port.
$DroppedPatches = @(
    "wasmvm.api.rs.patch.dropped",
    "wasmvm.lib.go.patch.dropped"
)

Write-Host "=== check-baseline-v3-wasmvm ===" -ForegroundColor Cyan
Write-Host "  wasmvm tag : $WasmvmTag"
Write-Host "  patch dir  : $PatchDir"
Write-Host "  build dir  : $BuildDir"
Write-Host ""

if (-not (Test-Path $BuildDir)) {
    New-Item -ItemType Directory -Path $BuildDir | Out-Null
}

if (-not (Test-Path $WasmvmDir)) {
    Write-Host "cloning wasmvm into $WasmvmDir..." -ForegroundColor Yellow
    & git clone --depth 1 --branch $WasmvmTag https://github.com/CosmWasm/wasmvm $WasmvmDir
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Failed to clone wasmvm $WasmvmTag"
        exit 2
    }
}

Push-Location $WasmvmDir
try {
    Write-Host "fetching tags into $WasmvmDir..."
    $null = & git fetch --depth 1 origin "refs/tags/${WasmvmTag}:refs/tags/${WasmvmTag}" 2>&1
    $null = & git reset --hard $WasmvmTag 2>&1
    $null = & git clean -fdx 2>&1
    $sha = (& git rev-parse --short HEAD).Trim()
    Write-Host "wasmvm checked out at $WasmvmTag ($sha)"
    Write-Host ""

    $clean    = 0
    $threeway = 0
    $conflict = 0

    foreach ($pname in $DroppedPatches) {
        $ppath = Join-Path $PatchDir $pname
        if (-not (Test-Path $ppath)) {
            Write-Host ("  MISSING   " + $pname) -ForegroundColor Red
            $conflict++
            continue
        }

        # git apply --check tries a strict apply
        $checkOutput = & git apply --check $ppath 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Host ("  CLEAN     " + $pname) -ForegroundColor Green
            $clean++
            continue
        }

        # try 3-way
        $threewayOutput = & git apply --3way --check $ppath 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Host ("  3WAY-OK   " + $pname) -ForegroundColor Yellow
            $threeway++
            continue
        }

        Write-Host ("  CONFLICT  " + $pname) -ForegroundColor Red
        Write-Host ("    strict-apply error: " + ($checkOutput -join " | ")) -ForegroundColor DarkGray
        $conflict++
    }

    Write-Host ""
    Write-Host "summary: $clean clean / $threeway 3-way-ok / $conflict conflicts (target=$WasmvmTag)" -ForegroundColor Cyan

    if ($conflict -gt 0) {
        Write-Host "→ Drift requires manual reanchor for at least one patch." -ForegroundColor Yellow
        exit 1
    } else {
        Write-Host "OK - dropped patches can be revived (modulo 3-way notes)." -ForegroundColor Green
        exit 0
    }
} finally { Pop-Location }
