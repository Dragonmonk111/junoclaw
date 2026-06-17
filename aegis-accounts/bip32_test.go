package aegisaccounts

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"math/big"
	"strings"
	"testing"
)

// base58Alphabet is the Bitcoin base58 alphabet.
const base58Alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"

// base58CheckDecode decodes a base58check string and verifies the trailing
// 4-byte double-SHA256 checksum, returning the payload without the checksum.
func base58CheckDecode(s string) ([]byte, error) {
	num := big.NewInt(0)
	base := big.NewInt(58)
	for _, c := range s {
		idx := strings.IndexRune(base58Alphabet, c)
		if idx < 0 {
			return nil, fmt.Errorf("invalid base58 char %q", c)
		}
		num.Mul(num, base)
		num.Add(num, big.NewInt(int64(idx)))
	}
	dec := num.Bytes()
	leading := 0
	for _, c := range s {
		if c == '1' {
			leading++
		} else {
			break
		}
	}
	full := append(make([]byte, leading), dec...)
	if len(full) < 4 {
		return nil, fmt.Errorf("base58: too short")
	}
	payload, checksum := full[:len(full)-4], full[len(full)-4:]
	first := sha256.Sum256(payload)
	second := sha256.Sum256(first[:])
	if !bytes.Equal(second[:4], checksum) {
		return nil, fmt.Errorf("base58check: bad checksum")
	}
	return payload, nil
}

// decodeXprv extracts (privKey, chainCode) from a canonical BIP-32 xprv.
// 78-byte payload layout: [0:4] version, [4] depth, [5:9] fingerprint,
// [9:13] child number, [13:45] chain code, [45:78] 0x00||privkey.
func decodeXprv(t *testing.T, xprv string) (priv, chain []byte) {
	t.Helper()
	payload, err := base58CheckDecode(xprv)
	if err != nil {
		t.Fatalf("decode xprv: %v", err)
	}
	if len(payload) != 78 {
		t.Fatalf("xprv payload len = %d, want 78", len(payload))
	}
	return payload[46:78], payload[13:45]
}

// BIP-32 reference Test Vector 1 (seed 000102...0f). Expected values are derived
// from the canonical xprv strings, each protected by its own base58check
// checksum (a typo fails loudly rather than silently passing), making this an
// authoritative, non-circular check that the classical HD derivation matches the
// standard bit-for-bit. This is what keeps the secp256k1 half wallet-compatible.
func TestBIP32Vector1(t *testing.T) {
	const (
		xprvM   = "xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvvNKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi"
		xprvM0H = "xprv9uHRZZhk6KAJC1avXpDAp4MDc3sQKNxDiPvvkX8Br5ngLNv1TxvUxt4cV1rGL5hj6KCesnDYUhd7oWgT11eZG7XnxHrnYeSvkzY7d2bhkJ7"
	)
	seed, _ := hex.DecodeString("000102030405060708090a0b0c0d0e0f")

	wantMKey, wantMChain := decodeXprv(t, xprvM)
	mKey, mChain, err := masterKey(seed)
	if err != nil {
		t.Fatalf("masterKey: %v", err)
	}
	if !bytes.Equal(mKey, wantMKey) {
		t.Fatalf("master key\n got  %x\n want %x", mKey, wantMKey)
	}
	if !bytes.Equal(mChain, wantMChain) {
		t.Fatalf("master chain\n got  %x\n want %x", mChain, wantMChain)
	}

	wantCKey, wantCChain := decodeXprv(t, xprvM0H)
	cKey, cChain, err := ckdPriv(mKey, mChain, HardenedOffset+0)
	if err != nil {
		t.Fatalf("ckdPriv m/0': %v", err)
	}
	if !bytes.Equal(cKey, wantCKey) {
		t.Fatalf("m/0' key\n got  %x\n want %x", cKey, wantCKey)
	}
	if !bytes.Equal(cChain, wantCChain) {
		t.Fatalf("m/0' chain\n got  %x\n want %x", cChain, wantCChain)
	}
}

func TestParseHDPath(t *testing.T) {
	idxs, err := ParseHDPath("m/44'/118'/0'/0/0")
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	want := []uint32{
		HardenedOffset + 44,
		HardenedOffset + 118,
		HardenedOffset + 0,
		0,
		0,
	}
	if len(idxs) != len(want) {
		t.Fatalf("len = %d, want %d", len(idxs), len(want))
	}
	for i := range want {
		if idxs[i] != want[i] {
			t.Fatalf("idx[%d] = %d, want %d", i, idxs[i], want[i])
		}
	}

	for _, bad := range []string{"", "44'/0", "n/0", "m/abc", "m/0/"} {
		if _, err := ParseHDPath(bad); err == nil {
			t.Fatalf("expected error parsing %q", bad)
		}
	}
}

func TestDeriveSecp256k1JunoPath(t *testing.T) {
	seed, _ := hex.DecodeString("000102030405060708090a0b0c0d0e0f")
	priv, err := DeriveSecp256k1(seed, DefaultHDPath)
	if err != nil {
		t.Fatalf("derive: %v", err)
	}
	if priv == nil {
		t.Fatal("nil private key")
	}
	// Deterministic: same seed + path => same key.
	priv2, err := DeriveSecp256k1(seed, DefaultHDPath)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(priv.Serialize(), priv2.Serialize()) {
		t.Fatal("derivation not deterministic")
	}
}
