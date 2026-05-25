# make-cosmwasm-bn254-fork.ps1
#
# Produces a fork-ready cosmwasm-bn254 directory by:
#   1. Cloning CosmWasm/cosmwasm at $CosmwasmTag (default v3.0.6)
#   2. Applying the wasmvm-fork/patches/v3.0.x/ series in order
#   3. Committing each patch as a separate commit (preserves authoring history)
#   4. Tagging the result $TagName (default v3.0.6-bn254)
#
# After this script succeeds, the user does the GitHub-side steps once:
#   a. Create empty repo at https://github.com/Dragonmonk111/cosmwasm-bn254
#      (Settings: Public, no README/LICENSE/.gitignore — empty)
#   b. cd $ForkDir
#   c. git remote add origin https://github.com/Dragonmonk111/cosmwasm-bn254.git
#   d. git push -u origin <default-branch>
#   e. git push origin v3.0.6-bn254
#
# The patch series in wasmvm-fork/patches/v3.0.x/ remains the canonical
# AUTHORING source-of-truth. The fork is a generated CONSUMER convenience.
# To re-baseline against a future cosmwasm tag, edit the patches and re-run
# this script.

param(
    [string]$CosmwasmTag = "v3.0.6",
    [string]$TagName     = "v3.0.6-bn254",
    [string]$ForkDir     = (Join-Path $env:USERPROFILE "junoclaw-build\cosmwasm-bn254-fork"),
    [switch]$Force       = $false
)

$ErrorActionPreference = "Stop"

$JunoclawRoot = (Get-Item $PSScriptRoot).Parent.Parent.FullName
$PatchDir     = Join-Path $JunoclawRoot "wasmvm-fork\patches\v3.0.x"

if (-not (Test-Path $PatchDir)) {
    Write-Error "Patch directory not found: $PatchDir"
    exit 1
}

if (Test-Path $ForkDir) {
    if (-not $Force) {
        Write-Error "Fork directory already exists: $ForkDir`n  Re-run with -Force to wipe and recreate, or pass -ForkDir <other-path>."
        exit 1
    }
    Write-Host "=== -Force set; removing existing $ForkDir ===" -ForegroundColor Yellow
    Remove-Item -Recurse -Force $ForkDir
}

$ForkParent = Split-Path -Parent $ForkDir
if (-not (Test-Path $ForkParent)) {
    New-Item -ItemType Directory -Path $ForkParent -Force | Out-Null
}

Write-Host "=== clone CosmWasm/cosmwasm at $CosmwasmTag ===" -ForegroundColor Cyan
& git clone --depth 1 --branch $CosmwasmTag https://github.com/CosmWasm/cosmwasm.git $ForkDir
if ($LASTEXITCODE -ne 0) { Write-Error "clone failed"; exit 2 }

Push-Location $ForkDir
try {
    # Unshallow so we can rewrite history if needed for the upstream PR.
    # git writes informational messages to stderr ("From https://...") which
    # PowerShell's $ErrorActionPreference=Stop interprets as fatal.  Redirect
    # stderr to $null and rely on LASTEXITCODE instead.
    Write-Host "=== unshallow clone ===" -ForegroundColor Cyan
    $prev = $ErrorActionPreference; $ErrorActionPreference = "Continue"
    & git fetch --unshallow 2>$null
    & git fetch --tags 2>$null
    $ErrorActionPreference = $prev

    # Checkout the tag as a working branch.
    $branchName = "bn254/$CosmwasmTag"
    Write-Host "=== checkout branch $branchName from $CosmwasmTag ===" -ForegroundColor Cyan
    & git checkout -b $branchName "tags/$CosmwasmTag"
    if ($LASTEXITCODE -ne 0) { Write-Error "checkout failed"; exit 2 }

    # Configure committer for the patch commits (uses local git config if set,
    # otherwise falls back to a generic identity that the user can amend later).
    $committerName  = (& git config user.name)
    $committerEmail = (& git config user.email)
    if ([string]::IsNullOrWhiteSpace($committerName)) {
        & git config user.name "Dragonmonk111"
    }
    if ([string]::IsNullOrWhiteSpace($committerEmail)) {
        & git config user.email "dragonmonk111@users.noreply.github.com"
    }

    Write-Host "=== apply patches (one commit per patch) ===" -ForegroundColor Cyan
    $patches = Get-ChildItem $PatchDir -Filter '*.patch' | Sort-Object Name
    $applied = 0
    foreach ($p in $patches) {
        Write-Host "  applying $($p.Name) ..." -NoNewline
        & git apply $p.FullName
        if ($LASTEXITCODE -ne 0) {
            Write-Host " FAILED" -ForegroundColor Red
            Write-Error "Patch $($p.Name) failed to apply against $CosmwasmTag"
            exit 3
        }
        & git add -A
        $msg = "bn254: $($p.BaseName)"
        & git commit -m $msg --quiet
        if ($LASTEXITCODE -ne 0) {
            Write-Host " COMMIT FAILED" -ForegroundColor Red
            Write-Error "Commit failed after $($p.Name)"
            exit 4
        }
        Write-Host " OK" -ForegroundColor Green
        $applied++
    }

    Write-Host "=== applied $applied / $($patches.Count) patches ===" -ForegroundColor Green

    Write-Host "=== tag $TagName ===" -ForegroundColor Cyan
    & git tag -a $TagName -m "BN254 host functions on top of cosmwasm $CosmwasmTag (Juno gov prop #374)"
    if ($LASTEXITCODE -ne 0) { Write-Error "tag failed"; exit 5 }

    Write-Host ""
    Write-Host "=== Done. Fork-ready directory: $ForkDir ===" -ForegroundColor Green
    Write-Host "Branch: $branchName"
    Write-Host "Tag:    $TagName"
    Write-Host ""
    Write-Host "Next steps (manual, GitHub-side):"
    Write-Host "  1. Create empty repo: https://github.com/Dragonmonk111/cosmwasm-bn254"
    Write-Host "     Settings: Public, no README/LICENSE/.gitignore (truly empty)."
    Write-Host "  2. cd '$ForkDir'"
    Write-Host "  3. git remote add origin https://github.com/Dragonmonk111/cosmwasm-bn254.git"
    Write-Host "  4. git push -u origin $branchName"
    Write-Host "  5. git push origin $TagName"
    Write-Host ""
    Write-Host "Consumers can then add to their Cargo.toml:"
    Write-Host "  [patch.crates-io]"
    Write-Host "  cosmwasm-std = { git = `"https://github.com/Dragonmonk111/cosmwasm-bn254`", tag = `"$TagName`" }"
    Write-Host "  cosmwasm-vm  = { git = `"https://github.com/Dragonmonk111/cosmwasm-bn254`", tag = `"$TagName`" }"
} finally { Pop-Location }
