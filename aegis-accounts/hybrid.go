// Package aegisaccounts is the Project Aegis Phase D / D2 standalone harness for
// hybrid post-quantum Cosmos accounts, implementing ADR-007: a key that is
// secp256k1 AND ML-DSA-44 (FIPS 204) at once, so an account is forgeable only if
// BOTH primitives are broken.
//
// It validates the ADR-007 crypto end to end (keygen, sign, verify, address
// derivation, HD-from-mnemonic) without forking the Cosmos SDK. The SDK fork
// (D3) ports this logic 1:1, exactly as secretconn/ + PORTING.md relate to the
// CometBFT fork in Phase C.
//
// Determinism note: ML-DSA-44 *verification* is integer-only and deterministic
// (the consensus-safe property, Aegis §6). ML-DSA *signing* may be hedged; the
// harness does not depend on signature-byte determinism, only on keygen
// determinism (NewKeyFromSeed) which is what HD backup requires.
package aegisaccounts

import (
	"crypto"
	"crypto/rand"
	"crypto/sha256"
	"errors"
	"fmt"

	"github.com/cloudflare/circl/sign/mldsa/mldsa44"
	"github.com/decred/dcrd/dcrec/secp256k1/v4"
	"github.com/decred/dcrd/dcrec/secp256k1/v4/ecdsa"
)

// Fixed sizes (SEC1 compressed secp256k1 + FIPS 204 ML-DSA-44).
const (
	Secp256k1PubKeyLen = 33
	Secp256k1SigLen    = 64
	MLDSA44PubKeyLen   = mldsa44.PublicKeySize // 1312
	MLDSA44SigLen      = mldsa44.SignatureSize // 2420

	// Address domain-separation tags (ADR-007 §D3). The classical-vs-hybrid
	// distinction in the tag guarantees a hybrid address can never collide with
	// a plain secp256k1 or plain ML-DSA address.
	AddressTagHybrid = "pqc/hybrid-secp256k1-mldsa44"
	AddressTagMLDSA  = "pqc/mldsa44"

	// AddressLen keeps the 20-byte Cosmos bech32 shape.
	AddressLen = 20

	// DefaultHRP is Juno's bech32 human-readable prefix.
	DefaultHRP = "juno"
)

// HybridPrivKey holds both private halves of a hybrid account key.
type HybridPrivKey struct {
	Secp  *secp256k1.PrivateKey
	MLDSA *mldsa44.PrivateKey
}

// HybridPubKey holds both public halves of a hybrid account key.
type HybridPubKey struct {
	Secp  *secp256k1.PublicKey
	MLDSA *mldsa44.PublicKey
}

// HybridSignature is the pair of signatures over the same message bytes.
type HybridSignature struct {
	Secp  []byte // 64 B, [R||S] low-S over sha256(msg)
	MLDSA []byte // 2420 B, ML-DSA-44 over msg
}

// NewHybridFromSeeds builds a hybrid private key from a 32-byte secp256k1 scalar
// seed and a 32-byte ML-DSA-44 keygen seed. Both halves are deterministic in
// their seeds, which is what makes HD-from-mnemonic reproducible.
func NewHybridFromSeeds(secpSeed []byte, mldsaSeed *[mldsa44.SeedSize]byte) (*HybridPrivKey, error) {
	if len(secpSeed) != 32 {
		return nil, fmt.Errorf("aegis: secp256k1 seed must be 32 bytes, got %d", len(secpSeed))
	}
	if mldsaSeed == nil {
		return nil, errors.New("aegis: nil ML-DSA seed")
	}
	secp := secp256k1.PrivKeyFromBytes(secpSeed)
	_, mldsaPriv := mldsa44.NewKeyFromSeed(mldsaSeed)
	return &HybridPrivKey{Secp: secp, MLDSA: mldsaPriv}, nil
}

// GenerateHybrid creates a fresh random hybrid key (non-deterministic). Useful
// for ephemeral keys; HD accounts should use NewHybridFromMnemonic.
func GenerateHybrid() (*HybridPrivKey, error) {
	secp, err := secp256k1.GeneratePrivateKey()
	if err != nil {
		return nil, fmt.Errorf("aegis: secp256k1 keygen: %w", err)
	}
	_, mldsaPriv, err := mldsa44.GenerateKey(rand.Reader)
	if err != nil {
		return nil, fmt.Errorf("aegis: ml-dsa-44 keygen: %w", err)
	}
	return &HybridPrivKey{Secp: secp, MLDSA: mldsaPriv}, nil
}

// PubKey returns the matching hybrid public key.
func (k *HybridPrivKey) PubKey() *HybridPubKey {
	return &HybridPubKey{
		Secp:  k.Secp.PubKey(),
		MLDSA: k.MLDSA.Public().(*mldsa44.PublicKey),
	}
}

// Sign produces a hybrid signature: secp256k1 over sha256(msg) and ML-DSA-44
// over the same msg bytes (ADR-007 §D2).
func (k *HybridPrivKey) Sign(msg []byte) (*HybridSignature, error) {
	// Classical half: Cosmos-style 64-byte [R||S], low-S, over sha256(msg).
	digest := sha256.Sum256(msg)
	compact := ecdsa.SignCompact(k.Secp, digest[:], true) // 65 B: [recovery||R||S]
	if len(compact) != 65 {
		return nil, fmt.Errorf("aegis: unexpected compact sig length %d", len(compact))
	}
	secpSig := make([]byte, Secp256k1SigLen)
	copy(secpSig, compact[1:]) // drop recovery byte

	// Post-quantum half: ML-DSA-44 over the raw message (it hashes internally).
	mldsaSig, err := k.MLDSA.Sign(rand.Reader, msg, crypto.Hash(0))
	if err != nil {
		return nil, fmt.Errorf("aegis: ml-dsa-44 sign: %w", err)
	}
	return &HybridSignature{Secp: secpSig, MLDSA: mldsaSig}, nil
}

// Verify accepts iff BOTH halves verify over msg (ADR-007 §D2). A break of one
// primitive alone is not sufficient to forge.
func (p *HybridPubKey) Verify(msg []byte, sig *HybridSignature) bool {
	if sig == nil || len(sig.Secp) != Secp256k1SigLen || len(sig.MLDSA) != MLDSA44SigLen {
		return false
	}
	if !p.verifySecp(msg, sig.Secp) {
		return false
	}
	return mldsa44.Scheme().Verify(p.MLDSA, msg, sig.MLDSA, nil)
}

func (p *HybridPubKey) verifySecp(msg, sig64 []byte) bool {
	digest := sha256.Sum256(msg)
	var r, s secp256k1.ModNScalar
	if overflow := r.SetByteSlice(sig64[:32]); overflow {
		return false
	}
	if overflow := s.SetByteSlice(sig64[32:]); overflow {
		return false
	}
	if r.IsZero() || s.IsZero() {
		return false
	}
	return ecdsa.NewSignature(&r, &s).Verify(digest[:], p.Secp)
}

// Bytes returns the algorithm-tagged concatenation secp(33) || mldsa(1312).
func (p *HybridPubKey) Bytes() ([]byte, error) {
	mldsaBytes, err := p.MLDSA.MarshalBinary()
	if err != nil {
		return nil, fmt.Errorf("aegis: marshal ml-dsa pubkey: %w", err)
	}
	out := make([]byte, 0, Secp256k1PubKeyLen+MLDSA44PubKeyLen)
	out = append(out, p.Secp.SerializeCompressed()...)
	out = append(out, mldsaBytes...)
	return out, nil
}

// Bytes returns the concatenation secp(64) || mldsa(2420).
func (s *HybridSignature) Bytes() []byte {
	out := make([]byte, 0, Secp256k1SigLen+MLDSA44SigLen)
	out = append(out, s.Secp...)
	out = append(out, s.MLDSA...)
	return out
}

// addressHash implements the Cosmos SDK crypto/address construction:
// sha256( sha256(typ) || key ).
func addressHash(typ string, key []byte) []byte {
	inner := sha256.Sum256([]byte(typ))
	h := sha256.New()
	h.Write(inner[:])
	h.Write(key)
	return h.Sum(nil)
}

// AddressBytes derives the 20-byte hybrid address (ADR-007 §D3), committing to
// BOTH public halves under a domain-separated tag.
func (p *HybridPubKey) AddressBytes() ([]byte, error) {
	key, err := p.Bytes()
	if err != nil {
		return nil, err
	}
	return addressHash(AddressTagHybrid, key)[:AddressLen], nil
}

// Bech32Address returns the bech32-encoded hybrid address under hrp (e.g.
// "juno").
func (p *HybridPubKey) Bech32Address(hrp string) (string, error) {
	addr, err := p.AddressBytes()
	if err != nil {
		return "", err
	}
	return EncodeBech32(hrp, addr)
}
