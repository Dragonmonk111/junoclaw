# regen-patch-01-v3.ps1
#
# One-shot patch generator for v3.0.x/01-cosmwasm-std.imports.rs.patch.
#
# Procedure:
#   1. Reset the cosmwasm checkout to a clean v3.0.1 state.
#   2. Edit packages/std/src/exports/imports.rs in place: insert BN254
#      extern "C" decls after ed25519_batch_verify, BN254 Api impls
#      after the ed25519_batch_verify impl block (before fn debug),
#      and the bn254_error_from_code helper at end of file.
#   3. Run `git diff` to capture the patch.
#   4. Save into wasmvm-fork/patches/v3.0.x/.
#   5. Reset the cosmwasm checkout to clean v3.0.1 again.
#   6. Verify the new patch applies cleanly via `git apply --check`.

param(
    [string]$CosmwasmTag = "v3.0.1",
    [string]$BuildDir    = (Join-Path $env:USERPROFILE "junoclaw-build")
)

$ErrorActionPreference = "Continue"

$JunoclawRoot  = (Get-Item $PSScriptRoot).Parent.Parent.FullName
$CosmwasmDir   = Join-Path $BuildDir "cosmwasm-bn254"
$RelPath       = "packages/std/src/exports/imports.rs"
$AbsTargetFile = Join-Path $CosmwasmDir $RelPath
$OutDir        = Join-Path $JunoclawRoot "wasmvm-fork\patches\v3.0.x"
$OutPatch      = Join-Path $OutDir "01-cosmwasm-std.imports.rs.patch"

if (-not (Test-Path $AbsTargetFile)) {
    Write-Error "ERROR: $AbsTargetFile not found. Run check-baseline-v3.ps1 first to clone."
    exit 1
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

# --- 1. Reset cosmwasm checkout to clean v3.0.1 ----------------------------
Push-Location $CosmwasmDir
try {
    $null = & git reset --hard $CosmwasmTag 2>&1
    $null = & git clean -fdx 2>&1
    Write-Host "cosmwasm reset to $CosmwasmTag"
} finally { Pop-Location }

# --- 2. Read the v3 file as lines ------------------------------------------
$lines = [System.IO.File]::ReadAllLines($AbsTargetFile)
Write-Host "v3 imports.rs has $($lines.Length) lines"

# --- 3. Build the three insertions ----------------------------------------
$bn254Externs = @(
    '    // -- BN254 (alt_bn128) host functions ----------------------------------',
    '    //',
    '    // Mirror the EIP-196/EIP-197 precompiles. Calling on a chain that',
    '    // does not enable `cosmwasm_2_3` surfaces as a "missing import" error',
    '    // at contract load time -- the desired behaviour for capability gating.',
    '    //',
    '    // Convention: `input_ptr` is a Region; `out_ptr` (where present) is a',
    '    // pre-allocated Region of the exact output size. Return value is a',
    '    // u32 status code defined alongside `do_bn254_*` in cosmwasm-vm.',
    '    #[cfg(feature = "cosmwasm_2_3")]',
    '    fn bn254_add(input_ptr: u32, out_ptr: u32) -> u32;',
    '    #[cfg(feature = "cosmwasm_2_3")]',
    '    fn bn254_scalar_mul(input_ptr: u32, out_ptr: u32) -> u32;',
    '    #[cfg(feature = "cosmwasm_2_3")]',
    '    fn bn254_pairing_equality(input_ptr: u32) -> u32;',
    ''
)

$bn254ApiImpls = @(
    '    #[cfg(feature = "cosmwasm_2_3")]',
    '    fn bn254_add(&self, input: &[u8]) -> Result<[u8; 64], VerificationError> {',
    '        if input.len() != 128 {',
    '            return Err(VerificationError::generic_err(',
    '                "bn254_add: input must be 128 bytes",',
    '            ));',
    '        }',
    '        let send_input = Region::from_slice(input);',
    '        let out: [u8; 64] = [0; 64];',
    '        let recv_out = Region::from_slice(&out);',
    '',
    '        let result = unsafe {',
    '            bn254_add(send_input.as_ptr() as u32, recv_out.as_ptr() as u32)',
    '        };',
    '        match result {',
    '            0 => Ok(out),',
    '            code => Err(bn254_error_from_code(code)),',
    '        }',
    '    }',
    '',
    '    #[cfg(feature = "cosmwasm_2_3")]',
    '    fn bn254_scalar_mul(&self, input: &[u8]) -> Result<[u8; 64], VerificationError> {',
    '        if input.len() != 96 {',
    '            return Err(VerificationError::generic_err(',
    '                "bn254_scalar_mul: input must be 96 bytes",',
    '            ));',
    '        }',
    '        let send_input = Region::from_slice(input);',
    '        let out: [u8; 64] = [0; 64];',
    '        let recv_out = Region::from_slice(&out);',
    '',
    '        let result = unsafe {',
    '            bn254_scalar_mul(send_input.as_ptr() as u32, recv_out.as_ptr() as u32)',
    '        };',
    '        match result {',
    '            0 => Ok(out),',
    '            code => Err(bn254_error_from_code(code)),',
    '        }',
    '    }',
    '',
    '    #[cfg(feature = "cosmwasm_2_3")]',
    '    fn bn254_pairing_equality(&self, input: &[u8]) -> Result<bool, VerificationError> {',
    '        if input.len() % 192 != 0 {',
    '            return Err(VerificationError::generic_err(format!(',
    '                "bn254_pairing_equality: input length {} is not a multiple of 192",',
    '                input.len()',
    '            )));',
    '        }',
    '        let send_input = Region::from_slice(input);',
    '        let result = unsafe { bn254_pairing_equality(send_input.as_ptr() as u32) };',
    '        match result {',
    '            0 => Ok(true),',
    '            1 => Ok(false),',
    '            code => Err(bn254_error_from_code(code)),',
    '        }',
    '    }',
    ''
)

$bn254Helper = @(
    '',
    '#[cfg(feature = "cosmwasm_2_3")]',
    'fn bn254_error_from_code(code: u32) -> VerificationError {',
    '    match code {',
    '        2 => VerificationError::generic_err("bn254: invalid input length"),',
    '        3 => VerificationError::generic_err("bn254: point not on curve"),',
    '        4 => VerificationError::generic_err("bn254: G2 point not in subgroup"),',
    '        5 => VerificationError::generic_err("bn254: invalid field element (>= p)"),',
    '        6 => VerificationError::generic_err("bn254: backend error"),',
    '        7 => VerificationError::generic_err("bn254: too many pairing pairs"),',
    '        c => VerificationError::generic_err(format!("bn254: unknown error code {c}")),',
    '    }',
    '}'
)

# --- 4. Find anchor lines dynamically (defensive against minor v3 drift) --
$anchorExternIdx = -1

for ($i = 0; $i -lt $lines.Length; $i++) {
    if ($anchorExternIdx -eq -1 -and $lines[$i] -match '^\s*fn ed25519_batch_verify\(messages_ptr: u32, signatures_ptr: u32, public_keys_ptr: u32\) -> u32;\s*$') {
        $anchorExternIdx = $i
    }
}

# Find end of `impl Api for ExternalApi` block: find the `}` at column 0 that
# closes the impl. Strategy: locate `fn debug(&self, message: &str)` then
# walk forward to its closing `}` (column 4) followed by `}` at column 0.
$debugImplIdx = -1
for ($i = 0; $i -lt $lines.Length; $i++) {
    if ($lines[$i] -match '^\s{4}fn debug\(&self, message: &str\)') {
        $debugImplIdx = $i
        break
    }
}

if ($anchorExternIdx -lt 0) { Write-Error "Could not find ed25519_batch_verify extern decl"; exit 2 }
if ($debugImplIdx    -lt 0) { Write-Error "Could not find fn debug impl";                exit 2 }

Write-Host "anchors: extern@$($anchorExternIdx + 1)  debug-impl@$($debugImplIdx + 1)"

# --- 5. Construct new file content ----------------------------------------
$newLines = New-Object System.Collections.Generic.List[string]

# Part 1: lines [0 .. anchorExternIdx]  (up to and including ed25519_batch_verify decl)
for ($i = 0; $i -le $anchorExternIdx; $i++) { $newLines.Add($lines[$i]) }

# Part 2: blank line that should follow (which is lines[anchorExternIdx + 1])
if ($lines[$anchorExternIdx + 1] -ne '') {
    Write-Error "Expected blank line after ed25519_batch_verify; found '$($lines[$anchorExternIdx + 1])'"
    exit 2
}
$newLines.Add($lines[$anchorExternIdx + 1])  # blank

# Part 3: BN254 extern decls
foreach ($l in $bn254Externs) { $newLines.Add($l) }

# Part 4: rest of the file from lines[anchorExternIdx + 2] up to lines[debugImplIdx - 1]
# (line before fn debug impl)
for ($i = $anchorExternIdx + 2; $i -lt $debugImplIdx; $i++) { $newLines.Add($lines[$i]) }

# Part 5: BN254 Api impls, then fn debug impl, then rest of file
foreach ($l in $bn254ApiImpls) { $newLines.Add($l) }
for ($i = $debugImplIdx; $i -lt $lines.Length; $i++) { $newLines.Add($lines[$i]) }

# Part 6: bn254_error_from_code helper appended at end
foreach ($l in $bn254Helper) { $newLines.Add($l) }

# Detect if original file ends with a trailing newline; preserve it
$originalEndsWithNewline = (Get-Content $AbsTargetFile -Raw).EndsWith("`n")
$newContent = ($newLines -join "`n")
if ($originalEndsWithNewline) { $newContent += "`n" }

# --- 6. Write modified file ------------------------------------------------
[System.IO.File]::WriteAllText($AbsTargetFile, $newContent, (New-Object System.Text.UTF8Encoding($false)))
Write-Host "wrote modified file ($($newLines.Count) lines)"

# --- 7. git diff -> patch --------------------------------------------------
Push-Location $CosmwasmDir
try {
    $diffOut = & git diff $RelPath
    if (-not $diffOut) {
        Write-Error "git diff produced empty output (no changes detected?)"
        exit 3
    }
    [System.IO.File]::WriteAllText($OutPatch, ($diffOut -join "`n") + "`n", (New-Object System.Text.UTF8Encoding($false)))
    Write-Host "wrote $OutPatch"

    # --- 8. Reset cosmwasm checkout ----------------------------------------
    $null = & git checkout -- $RelPath 2>&1
    Write-Host "reset $RelPath to clean v3.0.1"

    # --- 9. Verify the new patch applies cleanly ---------------------------
    $null = & git apply --check $OutPatch 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "VERIFY OK -- $OutPatch applies cleanly to $CosmwasmTag" -ForegroundColor Green
        exit 0
    } else {
        Write-Error "VERIFY FAILED -- new patch does not apply cleanly"
        exit 4
    }
} finally { Pop-Location }
