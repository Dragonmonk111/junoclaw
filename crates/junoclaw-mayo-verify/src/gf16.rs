//! GF(16) arithmetic and m-vector operations.
//!
//! MAYO operates over GF(16) = GF(2)[x] / (x^4 + x + 1).
//! Elements are packed as 4-bit nibbles in `u64` limbs (16 elements per limb).

use crate::params::ParameterSet;

/// GF(16) multiplication mod x^4 + x + 1.
#[inline(always)]
pub fn mul(a: u8, b: u8) -> u8 {
    let mut p = (a & 1) * b;
    p ^= (a & 2) * b;
    p ^= (a & 4) * b;
    p ^= (a & 8) * b;
    let top = p & 0xf0;
    ((p ^ (top >> 4) ^ (top >> 3)) & 0x0f) as u8
}

/// GF(16) addition / subtraction (same operation: XOR).
#[inline(always)]
pub fn add(a: u8, b: u8) -> u8 {
    a ^ b
}

/// Pre-computed multiplication table for vectorised ops.
/// Returns a `u32` where each byte is `mul(b, n)` for n = [1, 2, 4, 8].
#[inline(always)]
pub fn mul_table(b: u8) -> u32 {
    let x = (b as u32).wrapping_mul(0x08040201);
    let high = x & 0xf0f0f0f0;
    x ^ (high >> 4) ^ (high >> 3)
}

/// Multiply an m-vector (packed nibbles in `u64` limbs) by a GF(16) scalar
/// and XOR the result into `acc`.
#[inline(always)]
pub fn m_vec_mul_add<P: ParameterSet>(
    m_vec_limbs: usize,
    inp: &[u64],
    scalar: u8,
    acc: &mut [u64],
) {
    let tab = mul_table(scalar);
    let lsb_mask = 0x1111111111111111u64;
    for i in 0..m_vec_limbs {
        acc[i] ^= (inp[i] & lsb_mask).wrapping_mul(tab as u64 & 0xff)
            ^ ((inp[i] >> 1) & lsb_mask).wrapping_mul((tab >> 8) as u64 & 0xf)
            ^ ((inp[i] >> 2) & lsb_mask).wrapping_mul((tab >> 16) as u64 & 0xf)
            ^ ((inp[i] >> 3) & lsb_mask).wrapping_mul((tab >> 24) as u64 & 0xf);
    }
}

/// XOR two m-vectors.
#[inline(always)]
pub fn m_vec_add(m_vec_limbs: usize, a: &[u64], b: &mut [u64]) {
    for i in 0..m_vec_limbs {
        b[i] ^= a[i];
    }
}

/// Copy an m-vector.
#[inline(always)]
pub fn m_vec_copy(m_vec_limbs: usize, src: &[u64], dst: &mut [u64]) {
    for i in 0..m_vec_limbs {
        dst[i] = src[i];
    }
}

/// Multiply m-vector by x (the primitive element) and add into acc.
#[inline(always)]
pub fn m_vec_mul_add_x(m_vec_limbs: usize, inp: &[u64], acc: &mut [u64]) {
    let msb_mask = 0x8888888888888888u64;
    for i in 0..m_vec_limbs {
        let t = inp[i] & msb_mask;
        acc[i] ^= ((inp[i] ^ t) << 1) ^ ((t >> 3) * 3);
    }
}

/// Multiply m-vector by x^{-1} and add into acc.
#[inline(always)]
pub fn m_vec_mul_add_x_inv(m_vec_limbs: usize, inp: &[u64], acc: &mut [u64]) {
    let lsb_mask = 0x1111111111111111u64;
    for i in 0..m_vec_limbs {
        let t = inp[i] & lsb_mask;
        acc[i] ^= ((inp[i] ^ t) >> 1) ^ (t * 9);
    }
}

/// Combine 16 "bins" (accumulators indexed by GF(16) value) into a single
/// m-vector result. This is the bottleneck of MAYO verification.
#[inline(always)]
pub fn m_vec_multiply_bins<P: ParameterSet>(m_vec_limbs: usize, bins: &mut [u64], out: &mut [u64]) {
    // Fixed reduction pattern from reference C implementation.
    // Uses split_at_mut to satisfy the borrow checker.
    macro_rules! op {
        ($src:literal, $dst:literal, $f:ident) => {
            {
                let m = m_vec_limbs;
                let s = $src * m;
                let s_end = s + m;
                let d = $dst * m;
                let d_end = d + m;
                if s_end <= d {
                    let (left, right) = bins.split_at_mut(d);
                    let src = &left[s..s_end];
                    let dst = &mut right[..m];
                    $f(m, src, dst);
                } else if d_end <= s {
                    let (left, right) = bins.split_at_mut(s);
                    let src = &right[..m];
                    let dst = &mut left[d..d_end];
                    $f(m, src, dst);
                } else {
                    panic!("overlapping bins");
                }
            }
        };
    }

    op!(5, 10, m_vec_mul_add_x_inv);
    op!(11, 12, m_vec_mul_add_x);
    op!(10, 7, m_vec_mul_add_x_inv);
    op!(12, 6, m_vec_mul_add_x);
    op!(7, 14, m_vec_mul_add_x_inv);
    op!(6, 3, m_vec_mul_add_x);
    op!(14, 15, m_vec_mul_add_x_inv);
    op!(3, 8, m_vec_mul_add_x);
    op!(15, 13, m_vec_mul_add_x_inv);
    op!(8, 4, m_vec_mul_add_x);
    op!(13, 9, m_vec_mul_add_x_inv);
    op!(4, 2, m_vec_mul_add_x);
    op!(9, 1, m_vec_mul_add_x_inv);
    op!(2, 1, m_vec_mul_add_x);
    m_vec_copy(m_vec_limbs, &bins[m_vec_limbs..2 * m_vec_limbs], out);
}

/// Decode packed nibbles (2 per byte) into individual nibbles.
/// `out` must have at least `in_bytes * 2` capacity (or `in_bytes * 2 - 1` if `n` is odd).
pub fn decode(in_bytes: &[u8], out: &mut [u8], n: usize) {
    let full = n / 2;
    for i in 0..full {
        out[2 * i] = in_bytes[i] & 0x0f;
        out[2 * i + 1] = in_bytes[i] >> 4;
    }
    if n % 2 == 1 {
        out[n - 1] = in_bytes[full] & 0x0f;
    }
}

/// Encode individual nibbles into packed bytes.
pub fn encode(in_nibbles: &[u8], out: &mut [u8], n: usize) {
    let full = n / 2;
    for i in 0..full {
        out[i] = in_nibbles[2 * i] | (in_nibbles[2 * i + 1] << 4);
    }
    if n % 2 == 1 {
        out[full] = in_nibbles[n - 1];
    }
}

/// Matrix multiplication over GF(16).
/// `a` is row-major with `colrow_ab` columns; result in `c`.
pub fn mat_mul(a: &[u8], b: &[u8], c: &mut [u8], colrow_ab: usize, row_a: usize, col_b: usize) {
    for i in 0..row_a {
        for j in 0..col_b {
            let mut acc = 0u8;
            for k in 0..colrow_ab {
                acc = add(acc, mul(a[i * colrow_ab + k], b[k * col_b + j]));
            }
            c[i * col_b + j] = acc;
        }
    }
}

/// Matrix addition over GF(16).
pub fn mat_add(a: &[u8], b: &[u8], c: &mut [u8], rows: usize, cols: usize) {
    for i in 0..rows {
        for j in 0..cols {
            c[i * cols + j] = a[i * cols + j] ^ b[i * cols + j];
        }
    }
}
