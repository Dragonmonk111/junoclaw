//! MAYO signature verification.

use crate::error::Error;
use crate::gf16::{self, decode, m_vec_add, m_vec_multiply_bins};
use crate::params::ParameterSet;
use alloc::vec;
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// AES-128-CTR (counter in bytes 0..7 LE, nonce in bytes 8..15 = 0)
// ---------------------------------------------------------------------------

/// AES-128-CTR as used by MAYO-C.
/// Counter block layout: bytes 0..11 = IV (zeros), bytes 12..15 = 32-bit big-endian counter.
fn aes128_ctr(key: &[u8], out: &mut [u8]) {
    use aes::cipher::{BlockEncryptMut, KeyInit, generic_array::GenericArray};
    use aes::Aes128;
    let mut cipher = Aes128::new_from_slice(key).unwrap();
    let mut block = [0u8; 16];
    let mut counter: u32 = 0;
    for chunk in out.chunks_mut(16) {
        block[12..16].copy_from_slice(&counter.to_be_bytes());
        let mut ga = GenericArray::clone_from_slice(&block);
        cipher.encrypt_block_mut(&mut ga);
        for (o, b) in chunk.iter_mut().zip(ga.iter()) {
            *o = *b;
        }
        counter += 1;
    }
}

// ---------------------------------------------------------------------------
// SHAKE256
// ---------------------------------------------------------------------------

fn shake256(out: &mut [u8], data: &[u8]) {
    use sha3::{Shake256, digest::Update, digest::ExtendableOutput, digest::XofReader};
    let mut hasher = Shake256::default();
    hasher.update(data);
    let mut reader = hasher.finalize_xof();
    reader.read(out);
}

// ---------------------------------------------------------------------------
// Public-key expansion
// ---------------------------------------------------------------------------

/// Expand a compact MAYO public key into P1, P2, P3.
/// Returns `(P1, P2, P3)` as Vec<u64> limbs.
pub(crate) fn expand_pk<P: ParameterSet>(cpk: &[u8]) -> Result<(Vec<u64>, Vec<u64>, Vec<u64>), Error> {
    if cpk.len() != P::PK_BYTES {
        return Err(Error::InvalidLength {
            expected: P::PK_BYTES,
            actual: cpk.len(),
        });
    }

    let seed_pk = &cpk[..P::PK_SEED_BYTES];
    let packed_p3 = &cpk[P::PK_SEED_BYTES..];

    // P1 + P2 keystream length uses PACKED nibble sizes (m/2 bytes per m-vec),
    // not limb-storage sizes (m_vec_limbs*8 per m-vec). The two coincide only
    // for MAYO-2 (m=64); for m=78/108/142 the packed stream is shorter.
    let p1_vecs = P::V * (P::V + 1) / 2;
    let p2_vecs = P::V * P::O;
    let p1_packed_bytes = p1_vecs * (P::M / 2);
    let p2_packed_bytes = p2_vecs * (P::M / 2);
    let mut p1_p2_packed = vec![0u8; p1_packed_bytes + p2_packed_bytes];
    aes128_ctr(seed_pk, &mut p1_p2_packed);

    let p1_limbs = P::P1_BYTES / 8;
    let p2_limbs = P::P2_BYTES / 8;
    let p3_limbs = P::P3_BYTES / 8;
    let m_vec_limbs = P::M_VEC_LIMBS;

    let mut p1 = vec![0u64; p1_limbs];
    let mut p2 = vec![0u64; p2_limbs];
    let mut p3 = vec![0u64; p3_limbs];

    unpack_m_vecs(&p1_p2_packed, &mut p1, p1_limbs / m_vec_limbs, P::M);
    unpack_m_vecs(&p1_p2_packed[p1_packed_bytes..], &mut p2, p2_limbs / m_vec_limbs, P::M);
    unpack_m_vecs(packed_p3, &mut p3, p3_limbs / m_vec_limbs, P::M);

    Ok((p1, p2, p3))
}

/// Unpack m-vectors from packed bytes.
fn unpack_m_vecs(inp: &[u8], out: &mut [u64], vecs: usize, m: usize) {
    let m_vec_limbs = (m + 15) / 16;
    let half = m / 2;
    let copy_len = core::cmp::min(half, m_vec_limbs * 8);
    for i in 0..vecs {
        let src_off = i * half;
        let dst_off = i * m_vec_limbs;
        let mut tmp = [0u8; 72]; // max m_vec_limbs * 8 = 9*8 = 72
        tmp[..copy_len].copy_from_slice(&inp[src_off..src_off + copy_len]);
        for j in 0..m_vec_limbs {
            let mut val = 0u64;
            for k in 0..8 {
                val |= (tmp[j * 8 + k] as u64) << (k * 8);
            }
            out[dst_off + j] = val;
        }
    }
}

// ---------------------------------------------------------------------------
// Public map evaluation: S * P * S^t
// ---------------------------------------------------------------------------

/// Compute P * S^t.
/// Optimised: processes one output row at a time, reusing a `k * 16` bin accumulator
/// instead of allocating the full `n * k * 16` grid.  Peak memory drops from
/// ~165 KB to ~2 KB for MAYO-2.
fn calculate_ps<P: ParameterSet>(
    p1: &[u64],
    p2: &[u64],
    p3: &[u64],
    s: &[u8],
) -> Vec<u64> {
    let n = P::N;
    let v = P::V;
    let o = P::O;
    let k = P::K;
    let m_vec_limbs = P::M_VEC_LIMBS;

    let mut ps = vec![0u64; n * k * m_vec_limbs];
    let mut row_acc = vec![0u64; k * 16 * m_vec_limbs];

    let mut p1_used = 0usize;
    for row in 0..v {
        row_acc.fill(0);
        for j in row..v {
            for col in 0..k {
                let idx = (col * 16 + s[col * n + j] as usize) * m_vec_limbs;
                m_vec_add(m_vec_limbs, &p1[p1_used * m_vec_limbs..], &mut row_acc[idx..]);
            }
            p1_used += 1;
        }
        for j in 0..o {
            for col in 0..k {
                let p2_idx = (row * o + j) * m_vec_limbs;
                let idx = (col * 16 + s[col * n + j + v] as usize) * m_vec_limbs;
                m_vec_add(m_vec_limbs, &p2[p2_idx..], &mut row_acc[idx..]);
            }
        }
        for col in 0..k {
            let src = &mut row_acc[col * 16 * m_vec_limbs..];
            let dst = &mut ps[(row * k + col) * m_vec_limbs..];
            m_vec_multiply_bins::<P>(m_vec_limbs, src, dst);
        }
    }

    let mut p3_used = 0usize;
    for row in v..n {
        row_acc.fill(0);
        for j in row..n {
            for col in 0..k {
                let idx = (col * 16 + s[col * n + j] as usize) * m_vec_limbs;
                m_vec_add(m_vec_limbs, &p3[p3_used * m_vec_limbs..], &mut row_acc[idx..]);
            }
            p3_used += 1;
        }
        for col in 0..k {
            let src = &mut row_acc[col * 16 * m_vec_limbs..];
            let dst = &mut ps[(row * k + col) * m_vec_limbs..];
            m_vec_multiply_bins::<P>(m_vec_limbs, src, dst);
        }
    }

    ps
}

/// Compute S * PS.
/// Optimised: processes one (row, col) pair at a time with a 16-bin
/// accumulator on the stack (~256 B for MAYO-2) instead of a `k*k*16` grid.
fn calculate_sps<P: ParameterSet>(ps: &[u64], s: &[u8]) -> Vec<u64> {
    let n = P::N;
    let k = P::K;
    let m_vec_limbs = P::M_VEC_LIMBS;

    let mut sps = vec![0u64; k * k * m_vec_limbs];
    let mut bins = vec![0u64; 16 * m_vec_limbs];

    for row in 0..k {
        for col in 0..k {
            bins.fill(0);
            for j in 0..n {
                let ps_idx = (j * k + col) * m_vec_limbs;
                let acc_idx = (s[row * n + j] as usize) * m_vec_limbs;
                m_vec_add(m_vec_limbs, &ps[ps_idx..], &mut bins[acc_idx..]);
            }
            let dst = &mut sps[(row * k + col) * m_vec_limbs..];
            m_vec_multiply_bins::<P>(m_vec_limbs, &mut bins, dst);
        }
    }
    sps
}

// ---------------------------------------------------------------------------
// RHS computation (reduce mod f(X) and compare with target)
// ---------------------------------------------------------------------------

fn compute_rhs<P: ParameterSet>(v_pv: &mut [u64], t: &[u8], y: &mut [u8]) {
    let m = P::M;
    let k = P::K;
    let m_vec_limbs = P::M_VEC_LIMBS;
    let top_pos = ((m - 1) % 16) * 4;

    // zero out tail nibbles
    if m % 16 != 0 {
        let mask = (1u64 << ((m % 16) * 4)) - 1;
        for i in 0..k * k {
            v_pv[i * m_vec_limbs + m_vec_limbs - 1] &= mask;
        }
    }

    let mut temp = [0u64; 9]; // max m_vec_limbs across all parameter sets

    for i in (0..k).rev() {
        for j in i..k {
            // multiply temp by X (shift up 4 bits)
            let top = ((temp[m_vec_limbs - 1] >> top_pos) & 0xf) as u8;
            temp[m_vec_limbs - 1] <<= 4;
            for l in (0..m_vec_limbs - 1).rev() {
                temp[l + 1] ^= temp[l] >> 60;
                temp[l] <<= 4;
            }
            // reduce mod f(X) — write directly into temp as little-endian bytes
            for jj in 0..4 {
                let val = gf16::mul(top, P::F_TAIL[jj]);
                let byte_idx = jj / 2;
                let shift = if jj % 2 == 0 { 0 } else { 4 };
                let limb_idx = byte_idx / 8;
                let byte_in_limb = byte_idx % 8;
                let old_byte = ((temp[limb_idx] >> (byte_in_limb * 8)) & 0xFF) as u8;
                let new_byte = old_byte ^ (val << shift);
                temp[limb_idx] = (temp[limb_idx] & !(0xFFu64 << (byte_in_limb * 8)))
                    | ((new_byte as u64) << (byte_in_limb * 8));
            }

            // add vPv[i,k + j] and symmetric if i != j
            let base = (i * k + j) * m_vec_limbs;
            for l in 0..m_vec_limbs {
                temp[l] ^= v_pv[base + l];
                if i != j {
                    temp[l] ^= v_pv[(j * k + i) * m_vec_limbs + l];
                }
            }
        }
    }

    // y = t XOR temp (read temp as little-endian bytes)
    for i in (0..m).step_by(2) {
        let byte_idx = i / 2;
        let limb_idx = byte_idx / 8;
        let byte_in_limb = byte_idx % 8;
        let byte_val = ((temp[limb_idx] >> (byte_in_limb * 8)) & 0xFF) as u8;
        y[i] = t[i] ^ (byte_val & 0xf);
        if i + 1 < m {
            y[i + 1] = t[i + 1] ^ ((byte_val >> 4) & 0xf);
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level verify
// ---------------------------------------------------------------------------

/// Verify a MAYO signature.
///
/// # Errors
/// Returns `Error::InvalidLength` if any slice has the wrong size for the
/// chosen parameter set.
pub fn verify<P: ParameterSet>(
    message: &[u8],
    signature: &[u8],
    cpk: &[u8],
) -> Result<bool, Error> {
    if signature.len() != P::SIG_BYTES {
        return Err(Error::InvalidLength {
            expected: P::SIG_BYTES,
            actual: signature.len(),
        });
    }
    if cpk.len() != P::PK_BYTES {
        return Err(Error::InvalidLength {
            expected: P::PK_BYTES,
            actual: cpk.len(),
        });
    }

    // 1. Expand public key
    let (p1, p2, p3) = expand_pk::<P>(cpk)?;

    // 2. Hash message
    let mut digest = vec![0u8; P::DIGEST_BYTES];
    shake256(&mut digest, message);

    // 3. Parse signature: [packed_s: SIG_BYTES - SALT_BYTES] [salt: SALT_BYTES]
    // Note: reference C impl places salt at the END of the signature.
    let salt = &signature[P::SIG_BYTES - P::SALT_BYTES..];
    let packed_s = &signature[..P::SIG_BYTES - P::SALT_BYTES];

    // 4. Compute t = H(digest || salt)
    let mut t_enc = vec![0u8; P::M_BYTES];
    let mut to_hash = vec![0u8; P::DIGEST_BYTES + P::SALT_BYTES];
    to_hash[..P::DIGEST_BYTES].copy_from_slice(&digest);
    to_hash[P::DIGEST_BYTES..].copy_from_slice(salt);
    shake256(&mut t_enc, &to_hash);

    let mut t = vec![0u8; P::M];
    decode(&t_enc, &mut t, P::M);

    // 5. Decode signature vector s
    let mut s = vec![0u8; P::K * P::N];
    decode(packed_s, &mut s, P::K * P::N);

    // 6. Evaluate public map
    let ps = calculate_ps::<P>(&p1, &p2, &p3, &s);
    let mut sps = calculate_sps::<P>(&ps, &s);
    let mut y = vec![0u8; P::M];
    compute_rhs::<P>(&mut sps, &t, &mut y);

    // 7. Compare y == 0 (since compute_rhs XORs with t, y should be 0 for valid sig)
    // Actually compute_rhs computes: y = t XOR temp, where temp = eval(public_map)
    // For a valid signature, eval(public_map) should equal t, so y should be 0.
    Ok(y.iter().all(|&b| b == 0))
}
