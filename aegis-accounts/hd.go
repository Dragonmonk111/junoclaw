package aegisaccounts

// HD-from-mnemonic derivation (ADR-007 §D4). A single BIP-39 mnemonic backs up
// BOTH halves of a hybrid account:
//
//   - secp256k1 half: standard BIP-44 (m/44'/118'/account'/0/index, Juno coin
//     type 118), so any normal Cosmos wallet recovers the classical key.
//   - ML-DSA-44 half: ML-DSA has no BIP-32, so we derive a 32-byte keygen seed
//     deterministically via HKDF-SHA256 over the same BIP-39 seed, bound to the
//     HD path, then run ML-DSA-44 key generation.
//
// Same mnemonic + same path => same hybrid account, every time.

import (
	"crypto/hkdf"
	"crypto/sha256"
	"fmt"

	"github.com/cloudflare/circl/sign/mldsa/mldsa44"
	bip39 "github.com/cosmos/go-bip39"
)

const (
	// JunoCoinType is SLIP-0044 coin type 118 (the Cosmos Hub / Juno family).
	JunoCoinType = 118
	// DefaultHDPath is the first account, first address.
	DefaultHDPath = "m/44'/118'/0'/0/0"
	// mldsaHKDFInfoPrefix domain-separates the ML-DSA seed derivation from any
	// other use of the BIP-39 seed; the full HD path is appended for binding.
	mldsaHKDFInfoPrefix = "aegis/pqc/mldsa44/v1:"
)

// NewHybridFromMnemonic derives a hybrid private key from a BIP-39 mnemonic.
// passphrase is the optional BIP-39 25th word (may be ""). path is a BIP-44
// derivation path; pass DefaultHDPath for the canonical first account.
func NewHybridFromMnemonic(mnemonic, passphrase, path string) (*HybridPrivKey, error) {
	if !bip39.IsMnemonicValid(mnemonic) {
		return nil, fmt.Errorf("aegis: invalid BIP-39 mnemonic")
	}
	seed := bip39.NewSeed(mnemonic, passphrase) // 64 B

	secp, err := DeriveSecp256k1(seed, path)
	if err != nil {
		return nil, fmt.Errorf("aegis: classical HD derivation: %w", err)
	}

	mldsaSeedBytes, err := hkdf.Key(sha256.New, seed, nil, mldsaHKDFInfoPrefix+path, mldsa44.SeedSize)
	if err != nil {
		return nil, fmt.Errorf("aegis: ml-dsa HKDF: %w", err)
	}
	var mldsaSeed [mldsa44.SeedSize]byte
	copy(mldsaSeed[:], mldsaSeedBytes)
	_, mldsaPriv := mldsa44.NewKeyFromSeed(&mldsaSeed)

	return &HybridPrivKey{Secp: secp, MLDSA: mldsaPriv}, nil
}

// NewMnemonic generates a fresh BIP-39 mnemonic with the given entropy bit size
// (128 => 12 words, 256 => 24 words).
func NewMnemonic(entropyBits int) (string, error) {
	entropy, err := bip39.NewEntropy(entropyBits)
	if err != nil {
		return "", fmt.Errorf("aegis: entropy: %w", err)
	}
	mnemonic, err := bip39.NewMnemonic(entropy)
	if err != nil {
		return "", fmt.Errorf("aegis: mnemonic: %w", err)
	}
	return mnemonic, nil
}
