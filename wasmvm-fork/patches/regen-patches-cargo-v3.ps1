# regen-patches-cargo-v3.ps1
#
# Manually regenerates 04-cosmwasm-std.Cargo.toml.patch and
# 08-cosmwasm-vm.Cargo.toml.patch against cosmwasm v3.0.1.
#
# `git apply --3way` failed because the patches' index SHAs reference
# v2.2.7 blobs that don't exist in the v3 checkout. The deltas themselves
# are tiny (1-2 line additions) so we apply them via direct text insertion
# and then `git diff` to capture.

param(
    [string]$CosmwasmTag = "v3.0.1",
    [string]$BuildDir    = (Join-Path $env:USERPROFILE "junoclaw-build")
)

$ErrorActionPreference = "Continue"

$JunoclawRoot = (Get-Item $PSScriptRoot).Parent.Parent.FullName
$CosmwasmDir  = Join-Path $BuildDir "cosmwasm-bn254"
$DstDir       = Join-Path $JunoclawRoot "wasmvm-fork\patches\v3.0.x"

if (-not (Test-Path (Join-Path $CosmwasmDir ".git"))) {
    Write-Error "cosmwasm checkout missing; run check-baseline-v3.ps1 first"
    exit 1
}

New-Item -ItemType Directory -Force -Path $DstDir | Out-Null

# --- 1. Reset cosmwasm to clean v3.0.1 ------------------------------------
Push-Location $CosmwasmDir
try {
    $null = & git reset --hard $CosmwasmTag 2>&1
    $null = & git clean -fdx 2>&1
    Write-Host "cosmwasm reset to $CosmwasmTag"
} finally { Pop-Location }

# --- 2. Edit packages/std/Cargo.toml --------------------------------------
$stdToml = Join-Path $CosmwasmDir 'packages/std/Cargo.toml'
$stdLines = [System.IO.File]::ReadAllLines($stdToml)
$out = New-Object System.Collections.Generic.List[string]
$inserted_2_3 = $false
$inserted_dep = $false

for ($i = 0; $i -lt $stdLines.Length; $i++) {
    $out.Add($stdLines[$i])

    # Insert `cosmwasm_2_3 = [...]` after `cosmwasm_2_2 = [...]` line.
    if (-not $inserted_2_3 -and $stdLines[$i] -match '^cosmwasm_2_2\s*=\s*\["cosmwasm_2_1"\]\s*$') {
        $out.Add('cosmwasm_2_3 = ["cosmwasm_2_2", "dep:cosmwasm-crypto-bn254"]')
        $inserted_2_3 = $true
    }

    # Insert bn254 dep after `cosmwasm-crypto = { version = "3.0.1", path = "../crypto" }` line
    # (in [target.'cfg(not(target_arch = "wasm32"))'.dependencies] section).
    if (-not $inserted_dep -and $stdLines[$i] -match '^cosmwasm-crypto\s*=\s*\{\s*version\s*=\s*"3\.0\.1",\s*path\s*=\s*"\.\./crypto"\s*\}\s*$') {
        $out.Add('cosmwasm-crypto-bn254 = { version = "0.1.0", path = "../crypto-bn254", optional = true }')
        $inserted_dep = $true
    }
}

if (-not $inserted_2_3) { Write-Error "could not find cosmwasm_2_2 anchor in std/Cargo.toml"; exit 2 }
if (-not $inserted_dep) { Write-Error "could not find cosmwasm-crypto dep anchor in std/Cargo.toml"; exit 2 }

$origRaw = Get-Content $stdToml -Raw
$endsNewline = $origRaw.EndsWith("`n")
$newContent = ($out -join "`n")
if ($endsNewline) { $newContent += "`n" }
[System.IO.File]::WriteAllText($stdToml, $newContent, (New-Object System.Text.UTF8Encoding($false)))
Write-Host "edited packages/std/Cargo.toml (inserted 2 lines)"

# --- 3. Edit packages/vm/Cargo.toml ---------------------------------------
$vmToml = Join-Path $CosmwasmDir 'packages/vm/Cargo.toml'
$vmLines = [System.IO.File]::ReadAllLines($vmToml)
$out = New-Object System.Collections.Generic.List[string]
$inserted_vm_dep = $false

for ($i = 0; $i -lt $vmLines.Length; $i++) {
    $out.Add($vmLines[$i])

    # Insert bn254 dep after the cosmwasm-crypto = ... line in vm/Cargo.toml.
    if (-not $inserted_vm_dep -and $vmLines[$i] -match '^cosmwasm-crypto\s*=\s*\{\s*version\s*=\s*"3\.0\.1",\s*path\s*=\s*"\.\./crypto"\s*\}\s*$') {
        $out.Add('cosmwasm-crypto-bn254 = { version = "0.1.0", path = "../crypto-bn254" }')
        $inserted_vm_dep = $true
    }
}

if (-not $inserted_vm_dep) { Write-Error "could not find cosmwasm-crypto dep anchor in vm/Cargo.toml"; exit 2 }

$origRaw = Get-Content $vmToml -Raw
$endsNewline = $origRaw.EndsWith("`n")
$newContent = ($out -join "`n")
if ($endsNewline) { $newContent += "`n" }
[System.IO.File]::WriteAllText($vmToml, $newContent, (New-Object System.Text.UTF8Encoding($false)))
Write-Host "edited packages/vm/Cargo.toml (inserted 1 line)"

# --- 4. Capture diffs as patches -------------------------------------------
Push-Location $CosmwasmDir
try {
    $diff04 = & git diff -- 'packages/std/Cargo.toml'
    $diff08 = & git diff -- 'packages/vm/Cargo.toml'

    if (-not $diff04) { Write-Error "empty diff for 04"; exit 3 }
    if (-not $diff08) { Write-Error "empty diff for 08"; exit 3 }

    $patch04 = Join-Path $DstDir '04-cosmwasm-std.Cargo.toml.patch'
    $patch08 = Join-Path $DstDir '08-cosmwasm-vm.Cargo.toml.patch'
    [System.IO.File]::WriteAllText($patch04, ($diff04 -join "`n") + "`n", (New-Object System.Text.UTF8Encoding($false)))
    [System.IO.File]::WriteAllText($patch08, ($diff08 -join "`n") + "`n", (New-Object System.Text.UTF8Encoding($false)))
    Write-Host "wrote $patch04"
    Write-Host "wrote $patch08"

    # --- 5. Reset cosmwasm to clean v3.0.1 again ---------------------------
    $null = & git checkout -- 'packages/std/Cargo.toml' 'packages/vm/Cargo.toml' 2>&1
    Write-Host "reset Cargo.tomls to clean v3.0.1"

    # --- 6. Verify both patches apply cleanly ------------------------------
    $null = & git apply --check $patch04 2>&1
    $rc04 = $LASTEXITCODE
    $null = & git apply --check $patch08 2>&1
    $rc08 = $LASTEXITCODE

    if ($rc04 -eq 0 -and $rc08 -eq 0) {
        Write-Host "VERIFY OK -- 04 and 08 apply cleanly to $CosmwasmTag" -ForegroundColor Green
        exit 0
    } else {
        Write-Error "VERIFY FAILED -- 04 rc=$rc04, 08 rc=$rc08"
        exit 4
    }
} finally { Pop-Location }
