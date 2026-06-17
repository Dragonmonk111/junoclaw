package aegisaccounts

// BIP-32 hierarchical-deterministic derivation for the classical secp256k1 half
// of a hybrid account. Self-contained (HMAC-SHA512 + decred secp256k1 scalar
// math) and validated against the BIP-32 reference test vectors in
// bip32_test.go. This is the *classical* HD path; the post-quantum ML-DSA half
// is derived separately via HKDF (see hd.go), because ML-DSA has no BIP-32.

import (
	"crypto/hmac"
	"crypto/sha512"
	"encoding/binary"
	"errors"
	"fmt"
	"strconv"
	"strings"

	"github.com/decred/dcrd/dcrec/secp256k1/v4"
)

// HardenedOffset marks the boundary between normal and hardened child indices
// (BIP-32: indices >= 2^31 are hardened).
const HardenedOffset uint32 = 0x80000000

// masterKey derives the BIP-32 master key and chain code from a seed.
func masterKey(seed []byte) (key, chainCode []byte, err error) {
	mac := hmac.New(sha512.New, []byte("Bitcoin seed"))
	mac.Write(seed)
	sum := mac.Sum(nil)
	il, ir := sum[:32], sum[32:]

	var sc secp256k1.ModNScalar
	if overflow := sc.SetByteSlice(il); overflow || sc.IsZero() {
		return nil, nil, errors.New("bip32: invalid master key (IL >= n or zero)")
	}
	return il, ir, nil
}

// ckdPriv performs a single BIP-32 private child-key derivation step.
func ckdPriv(kpar, cpar []byte, index uint32) (key, chainCode []byte, err error) {
	var data []byte
	if index >= HardenedOffset {
		// Hardened: 0x00 || ser256(kpar) || ser32(index)
		data = make([]byte, 0, 1+32+4)
		data = append(data, 0x00)
		data = append(data, kpar...)
	} else {
		// Normal: serP(point(kpar)) || ser32(index)
		pub := secp256k1.PrivKeyFromBytes(kpar).PubKey().SerializeCompressed()
		data = make([]byte, 0, 33+4)
		data = append(data, pub...)
	}
	var idx [4]byte
	binary.BigEndian.PutUint32(idx[:], index)
	data = append(data, idx[:]...)

	mac := hmac.New(sha512.New, cpar)
	mac.Write(data)
	sum := mac.Sum(nil)
	il, ir := sum[:32], sum[32:]

	var ilScalar, kparScalar secp256k1.ModNScalar
	if overflow := ilScalar.SetByteSlice(il); overflow {
		return nil, nil, errors.New("bip32: IL >= n; caller should try next index")
	}
	kparScalar.SetByteSlice(kpar)
	ilScalar.Add(&kparScalar) // (IL + kpar) mod n
	if ilScalar.IsZero() {
		return nil, nil, errors.New("bip32: derived key is zero; caller should try next index")
	}
	childKey := ilScalar.Bytes()
	return childKey[:], ir, nil
}

// ParseHDPath parses a string like "m/44'/118'/0'/0/0" into a slice of child
// indices, applying the hardened offset to elements suffixed with ' / h / H.
func ParseHDPath(path string) ([]uint32, error) {
	parts := strings.Split(strings.TrimSpace(path), "/")
	if len(parts) == 0 || parts[0] != "m" {
		return nil, fmt.Errorf("bip32: path must start with 'm', got %q", path)
	}
	out := make([]uint32, 0, len(parts)-1)
	for _, p := range parts[1:] {
		if p == "" {
			return nil, fmt.Errorf("bip32: empty path element in %q", path)
		}
		hardened := false
		if last := p[len(p)-1]; last == '\'' || last == 'h' || last == 'H' {
			hardened = true
			p = p[:len(p)-1]
		}
		v, err := strconv.ParseUint(p, 10, 32)
		if err != nil {
			return nil, fmt.Errorf("bip32: bad path element %q: %w", p, err)
		}
		idx := uint32(v)
		if idx >= HardenedOffset {
			return nil, fmt.Errorf("bip32: path element %d out of range", v)
		}
		if hardened {
			idx += HardenedOffset
		}
		out = append(out, idx)
	}
	return out, nil
}

// DeriveSecp256k1 derives a secp256k1 private key from a BIP-39 seed along the
// given BIP-32/44 path.
func DeriveSecp256k1(seed []byte, path string) (*secp256k1.PrivateKey, error) {
	key, chainCode, err := masterKey(seed)
	if err != nil {
		return nil, err
	}
	indices, err := ParseHDPath(path)
	if err != nil {
		return nil, err
	}
	for _, idx := range indices {
		key, chainCode, err = ckdPriv(key, chainCode, idx)
		if err != nil {
			return nil, err
		}
	}
	return secp256k1.PrivKeyFromBytes(key), nil
}
