# apply-and-test-v3.ps1
#
# Apply the v3.0.x patch series to a clean cosmwasm v3.0.1 checkout, then
# run `cargo test -p cosmwasm-crypto-bn254` and `cargo test -p cosmwasm-vm`.
# Best-effort verification of the day-2 forward-port deliverable.

param(
    [string]$CosmwasmTag = "v3.0.6",
    [string]$BuildDir    = (Join-Path $env:USERPROFILE "junoclaw-build"),
    [switch]$SkipTests   = $false
)

$ErrorActionPreference = "Continue"

$JunoclawRoot = (Get-Item $PSScriptRoot).Parent.Parent.FullName
$CosmwasmDir  = Join-Path $BuildDir "cosmwasm-bn254"
$PatchDir     = Join-Path $JunoclawRoot "wasmvm-fork\patches\v3.0.x"

Push-Location $CosmwasmDir
try {
    Write-Host "=== reset to clean v3.0.1 ===" -ForegroundColor Cyan
    $null = & git reset --hard $CosmwasmTag 2>&1
    $null = & git clean -fdx 2>&1
    Write-Host "  reset OK ($(& git rev-parse --short HEAD))"

    Write-Host "=== apply v3.0.x patch series ===" -ForegroundColor Cyan
    $patches = Get-ChildItem $PatchDir -Filter '*.patch' | Sort-Object Name
    $applied = 0
    $failed  = @()
    foreach ($p in $patches) {
        $null = & git apply $p.FullName 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Host ("  applied  " + $p.Name) -ForegroundColor Green
            $applied++
        } else {
            Write-Host ("  FAILED   " + $p.Name) -ForegroundColor Red
            $failed += $p.Name
        }
    }
    Write-Host "  applied: $applied / $($patches.Count)"
    if ($failed.Count -gt 0) {
        Write-Error "Patch application failed for: $($failed -join ', ')"
        exit 2
    }

    if ($SkipTests) {
        Write-Host "=== --SkipTests set; stopping here ===" -ForegroundColor Yellow
        exit 0
    }

    Write-Host "=== cargo test -p cosmwasm-crypto-bn254 (no-default-features) ===" -ForegroundColor Cyan
    $cryptoLog = Join-Path $BuildDir "cargo-test-crypto-bn254-v3.log"
    & cargo test -p cosmwasm-crypto-bn254 --no-default-features 2>&1 | Tee-Object -FilePath $cryptoLog
    $rcCrypto = $LASTEXITCODE

    Write-Host ""
    Write-Host "=== cargo test -p cosmwasm-vm (default features) ===" -ForegroundColor Cyan
    $vmLog = Join-Path $BuildDir "cargo-test-vm-v3.log"
    & cargo test -p cosmwasm-vm 2>&1 | Tee-Object -FilePath $vmLog
    $rcVm = $LASTEXITCODE

    Write-Host ""
    Write-Host "=== Summary ===" -ForegroundColor Cyan
    Write-Host "  cosmwasm-crypto-bn254 : rc=$rcCrypto  log=$cryptoLog"
    Write-Host "  cosmwasm-vm           : rc=$rcVm      log=$vmLog"

    if ($rcCrypto -eq 0 -and $rcVm -eq 0) {
        Write-Host "OK -- both test suites passed against patched v3.0.1" -ForegroundColor Green
        exit 0
    } else {
        Write-Error "One or both suites failed; inspect logs"
        exit 3
    }
} finally { Pop-Location }
