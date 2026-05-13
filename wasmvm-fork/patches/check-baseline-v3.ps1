# check-baseline-v3.ps1
#
# Windows-native PowerShell equivalent of check-baseline-v3.sh. Same one-question
# answer (do the v2.2.7 patches still apply cleanly to a v3.x cosmwasm tag?)
# but uses Windows path conventions and Git-for-Windows.
#
# Usage (from junoclaw repo root):
#   powershell -ExecutionPolicy Bypass -File wasmvm-fork\patches\check-baseline-v3.ps1
#   powershell -ExecutionPolicy Bypass -File wasmvm-fork\patches\check-baseline-v3.ps1 -CosmwasmTag v3.0.0
#
# Exit codes mirror the bash version: 0 clean, 1 precondition, 2 clone fail, 3 conflicts.

param(
    [string]$CosmwasmTag = "v3.0.1",
    [string]$PatchDir    = "$PSScriptRoot\v2.2.7",
    [string]$BuildDir    = (Join-Path $env:USERPROFILE "junoclaw-build")
)

$ErrorActionPreference = "Continue"
$JunoclawRoot = (Get-Item $PSScriptRoot).Parent.Parent.FullName
$CosmwasmDir  = Join-Path $BuildDir "cosmwasm-bn254"

if (-not (Test-Path (Join-Path $JunoclawRoot "Cargo.toml"))) {
    Write-Error "ERROR: junoclaw root not detected at $JunoclawRoot"
    exit 1
}

Write-Host "=== check-baseline-v3 ===" -ForegroundColor Cyan
Write-Host "  cosmwasm tag : $CosmwasmTag"
Write-Host "  patch dir    : $PatchDir"
Write-Host "  build dir    : $BuildDir"
Write-Host ""

if (-not (Test-Path $PatchDir)) {
    Write-Error "ERROR: patch directory not found: $PatchDir"
    exit 1
}

New-Item -ItemType Directory -Force -Path $BuildDir | Out-Null

# ----- 1. Clone or update upstream cosmwasm --------------------------------

if (Test-Path (Join-Path $CosmwasmDir ".git")) {
    Write-Host "fetching tags into $CosmwasmDir..."
    Push-Location $CosmwasmDir
    try {
        & git fetch --tags --depth=1 origin "tag" $CosmwasmTag 2>&1 | Out-Null
    } finally { Pop-Location }
} else {
    Write-Host "cloning cosmwasm into $CosmwasmDir..."
    & git clone --quiet "https://github.com/CosmWasm/cosmwasm" $CosmwasmDir
    if ($LASTEXITCODE -ne 0) { exit 2 }
}

Push-Location $CosmwasmDir
try {
    & git fetch --tags --depth=1 origin "tag" $CosmwasmTag 2>&1 | Out-Null
    & git reset --hard $CosmwasmTag 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Error "ERROR: failed to reset cosmwasm checkout to $CosmwasmTag"
        exit 2
    }
    & git clean -fdx 2>&1 | Out-Null
    $shortSha = (& git rev-parse --short HEAD).Trim()
    Write-Host "cosmwasm checked out at $CosmwasmTag ($shortSha)"
} finally { Pop-Location }
Write-Host ""

# ----- 2. git apply --check per patch --------------------------------------

$Clean       = @()
$ThreeWayOk  = @()
$Conflicts   = @()

$patches = Get-ChildItem -Path $PatchDir -Filter "*.patch" | Sort-Object Name

foreach ($p in $patches) {
    $name = $p.Name
    $patchAbs = $p.FullName

    Push-Location $CosmwasmDir
    try {
        $null = & git apply --check $patchAbs 2>&1
        $cleanRc = $LASTEXITCODE
    } finally { Pop-Location }

    if ($cleanRc -eq 0) {
        Write-Host ("  CLEAN     " + $name) -ForegroundColor Green
        $Clean += $name
        continue
    }

    Push-Location $CosmwasmDir
    try {
        $null = & git apply --check --3way $patchAbs 2>&1
        $threewayRc = $LASTEXITCODE
    } finally { Pop-Location }

    if ($threewayRc -eq 0) {
        Write-Host ("  3WAY-OK   " + $name + "    (clean 3-way merge possible)") -ForegroundColor Yellow
        $ThreeWayOk += $name
    } else {
        Write-Host ("  CONFLICT  " + $name + "    (manual rewrite required)") -ForegroundColor Red
        $Conflicts += $name
    }
}

Write-Host ""
Write-Host "summary: $($Clean.Count) clean / $($ThreeWayOk.Count) 3-way-ok / $($Conflicts.Count) conflicts (target=$CosmwasmTag)"

if ($Conflicts.Count -gt 0) {
    Write-Host ""
    Write-Host 'Conflicts (failed patches - rewrite needed):' -ForegroundColor Red
    foreach ($n in $Conflicts) { Write-Host "  - $n" }
    Write-Host ''
    Write-Host 'To inspect a specific conflict:'
    Write-Host '  Push-Location <cosmwasm-dir>; git apply --3way <patch>; Pop-Location'
    exit 3
}

if ($ThreeWayOk.Count -gt 0) {
    Write-Host ""
    Write-Host '3-way merge candidates (may apply with --3way but need review):' -ForegroundColor Yellow
    foreach ($n in $ThreeWayOk) { Write-Host "  - $n" }
}

Write-Host "OK - patch series can be applied to $CosmwasmTag (modulo any 3-way notes above)." -ForegroundColor Green
exit 0
