package aegisaccounts

import (
	"bytes"
	"strings"
	"testing"
)

// Canonical BIP-39 test mnemonic (all-zero entropy, 12 words).
const testMnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

func mustPubBytes(t *testing.T, p *HybridPubKey) []byte {
	t.Helper()
	b, err := p.Bytes()
	if err != nil {
		t.Fatalf("pubkey bytes: %v", err)
	}
	return b
}

func TestHybridSignVerifyRoundTrip(t *testing.T) {
	k, err := GenerateHybrid()
	if err != nil {
		t.Fatal(err)
	}
	msg := []byte("aegis :: send 100ujuno to juno1agentdao")
	sig, err := k.Sign(msg)
	if err != nil {
		t.Fatal(err)
	}
	if !k.PubKey().Verify(msg, sig) {
		t.Fatal("valid hybrid signature rejected")
	}
}

func TestHybridTamperedMessageFails(t *testing.T) {
	k, _ := GenerateHybrid()
	msg := []byte("transfer 1 to alice")
	sig, _ := k.Sign(msg)
	if k.PubKey().Verify([]byte("transfer 1000000 to mallory"), sig) {
		t.Fatal("tampered message accepted")
	}
}

// The headline ADR-007 property: forgery requires breaking BOTH primitives. A
// signature with a valid classical half but a foreign PQ half (or vice versa)
// must be rejected.
func TestHybridBothHalvesRequired(t *testing.T) {
	keyA, _ := GenerateHybrid()
	keyB, _ := GenerateHybrid()
	msg := []byte("the same canonical SignDoc bytes")

	sigA, _ := keyA.Sign(msg)
	sigB, _ := keyB.Sign(msg)
	pubA := keyA.PubKey()

	if !pubA.Verify(msg, sigA) {
		t.Fatal("legitimate signature rejected")
	}

	// A's classical half + B's PQ half → reject (PQ half is not A's).
	forgedPQ := &HybridSignature{Secp: sigA.Secp, MLDSA: sigB.MLDSA}
	if pubA.Verify(msg, forgedPQ) {
		t.Fatal("accepted signature with foreign ML-DSA half")
	}

	// B's classical half + A's PQ half → reject (classical half is not A's).
	forgedClassical := &HybridSignature{Secp: sigB.Secp, MLDSA: sigA.MLDSA}
	if pubA.Verify(msg, forgedClassical) {
		t.Fatal("accepted signature with foreign secp256k1 half")
	}
}

func TestHybridWrongPubKeyFails(t *testing.T) {
	keyA, _ := GenerateHybrid()
	keyB, _ := GenerateHybrid()
	msg := []byte("hello")
	sigA, _ := keyA.Sign(msg)
	if keyB.PubKey().Verify(msg, sigA) {
		t.Fatal("signature verified under the wrong public key")
	}
}

func TestHybridMalformedSignatureRejected(t *testing.T) {
	k, _ := GenerateHybrid()
	msg := []byte("x")
	sig, _ := k.Sign(msg)
	pub := k.PubKey()

	if pub.Verify(msg, &HybridSignature{Secp: sig.Secp[:10], MLDSA: sig.MLDSA}) {
		t.Fatal("accepted short secp half")
	}
	if pub.Verify(msg, &HybridSignature{Secp: sig.Secp, MLDSA: sig.MLDSA[:10]}) {
		t.Fatal("accepted short ml-dsa half")
	}
	if pub.Verify(msg, nil) {
		t.Fatal("accepted nil signature")
	}
}

func TestHybridSizes(t *testing.T) {
	k, _ := GenerateHybrid()
	pub := k.PubKey()
	pb := mustPubBytes(t, pub)
	if len(pb) != Secp256k1PubKeyLen+MLDSA44PubKeyLen {
		t.Fatalf("pubkey bytes = %d, want %d", len(pb), Secp256k1PubKeyLen+MLDSA44PubKeyLen)
	}
	sig, _ := k.Sign([]byte("m"))
	if len(sig.Bytes()) != Secp256k1SigLen+MLDSA44SigLen {
		t.Fatalf("sig bytes = %d, want %d", len(sig.Bytes()), Secp256k1SigLen+MLDSA44SigLen)
	}
	if MLDSA44PubKeyLen != 1312 || MLDSA44SigLen != 2420 {
		t.Fatalf("unexpected ML-DSA-44 sizes pub=%d sig=%d", MLDSA44PubKeyLen, MLDSA44SigLen)
	}
}

func TestHybridAddressShape(t *testing.T) {
	k, _ := GenerateHybrid()
	pub := k.PubKey()
	addr, err := pub.AddressBytes()
	if err != nil {
		t.Fatal(err)
	}
	if len(addr) != AddressLen {
		t.Fatalf("address len = %d, want %d", len(addr), AddressLen)
	}
	bech, err := pub.Bech32Address(DefaultHRP)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(bech, DefaultHRP+"1") {
		t.Fatalf("address %q lacks %q prefix", bech, DefaultHRP+"1")
	}
	// Bech32 decodes back to the same 20 bytes.
	_, decoded, err := DecodeBech32(bech)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(decoded, addr) {
		t.Fatal("bech32 address does not round-trip to address bytes")
	}
}

func TestHybridMnemonicDeterministic(t *testing.T) {
	k1, err := NewHybridFromMnemonic(testMnemonic, "", DefaultHDPath)
	if err != nil {
		t.Fatal(err)
	}
	k2, err := NewHybridFromMnemonic(testMnemonic, "", DefaultHDPath)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(k1.Secp.Serialize(), k2.Secp.Serialize()) {
		t.Fatal("secp256k1 half not deterministic from mnemonic")
	}
	if !bytes.Equal(mustPubBytes(t, k1.PubKey()), mustPubBytes(t, k2.PubKey())) {
		t.Fatal("hybrid pubkey not deterministic from mnemonic")
	}
	// And the derived account can sign/verify.
	msg := []byte("deterministic account works")
	sig, _ := k1.Sign(msg)
	if !k2.PubKey().Verify(msg, sig) {
		t.Fatal("cross-instance verify failed for same mnemonic")
	}
}

func TestHybridPassphraseChangesAccount(t *testing.T) {
	k1, _ := NewHybridFromMnemonic(testMnemonic, "", DefaultHDPath)
	k2, _ := NewHybridFromMnemonic(testMnemonic, "trezor", DefaultHDPath)
	if bytes.Equal(mustPubBytes(t, k1.PubKey()), mustPubBytes(t, k2.PubKey())) {
		t.Fatal("BIP-39 passphrase did not change the derived account")
	}
}

func TestHybridHDPathIndependence(t *testing.T) {
	k0, _ := NewHybridFromMnemonic(testMnemonic, "", "m/44'/118'/0'/0/0")
	k1, _ := NewHybridFromMnemonic(testMnemonic, "", "m/44'/118'/0'/0/1")

	if bytes.Equal(k0.Secp.Serialize(), k1.Secp.Serialize()) {
		t.Fatal("different HD index produced the same secp256k1 key")
	}
	a0, _ := k0.PubKey().AddressBytes()
	a1, _ := k1.PubKey().AddressBytes()
	if bytes.Equal(a0, a1) {
		t.Fatal("different HD index produced the same address")
	}
	// The ML-DSA half must also differ (HKDF info binds the path).
	m0, _ := k0.PubKey().MLDSA.MarshalBinary()
	m1, _ := k1.PubKey().MLDSA.MarshalBinary()
	if bytes.Equal(m0, m1) {
		t.Fatal("different HD index produced the same ML-DSA key")
	}
}

func TestHybridAddressCollisionResistance(t *testing.T) {
	seen := make(map[string]bool)
	for i := 0; i < 50; i++ {
		k, _ := GenerateHybrid()
		addr, _ := k.PubKey().AddressBytes()
		key := string(addr)
		if seen[key] {
			t.Fatal("address collision across distinct keys")
		}
		seen[key] = true
	}
}

func TestHybridSeedsDeterministic(t *testing.T) {
	secpSeed := bytes.Repeat([]byte{0x11}, 32)
	var mldsaSeed [32]byte
	for i := range mldsaSeed {
		mldsaSeed[i] = 0x22
	}
	k1, err := NewHybridFromSeeds(secpSeed, &mldsaSeed)
	if err != nil {
		t.Fatal(err)
	}
	k2, err := NewHybridFromSeeds(secpSeed, &mldsaSeed)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(mustPubBytes(t, k1.PubKey()), mustPubBytes(t, k2.PubKey())) {
		t.Fatal("NewHybridFromSeeds not deterministic")
	}
}

func TestNewMnemonicValid(t *testing.T) {
	for _, bits := range []int{128, 256} {
		mn, err := NewMnemonic(bits)
		if err != nil {
			t.Fatalf("NewMnemonic(%d): %v", bits, err)
		}
		if _, err := NewHybridFromMnemonic(mn, "", DefaultHDPath); err != nil {
			t.Fatalf("generated mnemonic unusable: %v", err)
		}
	}
}

func TestInvalidMnemonicRejected(t *testing.T) {
	if _, err := NewHybridFromMnemonic("not a valid mnemonic at all", "", DefaultHDPath); err == nil {
		t.Fatal("expected invalid-mnemonic error")
	}
}
