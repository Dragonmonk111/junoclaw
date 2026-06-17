package aegisaccounts

import (
	"bytes"
	"crypto/rand"
	"testing"
)

// BIP-173 reference vector: "A12UEL5L" is a valid bech32 string with hrp "a"
// and empty data. This independently validates the polymod/checksum logic.
func TestBech32ReferenceVector(t *testing.T) {
	hrp, data, err := DecodeBech32("A12UEL5L")
	if err != nil {
		t.Fatalf("decode reference vector: %v", err)
	}
	if hrp != "a" {
		t.Fatalf("hrp = %q, want %q", hrp, "a")
	}
	if len(data) != 0 {
		t.Fatalf("data len = %d, want 0", len(data))
	}
	got, err := EncodeBech32("a", nil)
	if err != nil {
		t.Fatalf("encode: %v", err)
	}
	if got != "a12uel5l" {
		t.Fatalf("re-encode = %q, want %q", got, "a12uel5l")
	}
}

func TestBech32RoundTrip(t *testing.T) {
	for i := 0; i < 200; i++ {
		orig := make([]byte, AddressLen)
		if _, err := rand.Read(orig); err != nil {
			t.Fatal(err)
		}
		enc, err := EncodeBech32(DefaultHRP, orig)
		if err != nil {
			t.Fatalf("encode: %v", err)
		}
		hrp, dec, err := DecodeBech32(enc)
		if err != nil {
			t.Fatalf("decode %q: %v", enc, err)
		}
		if hrp != DefaultHRP {
			t.Fatalf("hrp = %q, want %q", hrp, DefaultHRP)
		}
		if !bytes.Equal(dec, orig) {
			t.Fatalf("round-trip mismatch:\n got  %x\n want %x", dec, orig)
		}
	}
}

func TestBech32BadChecksumRejected(t *testing.T) {
	enc, err := EncodeBech32(DefaultHRP, make([]byte, AddressLen))
	if err != nil {
		t.Fatal(err)
	}
	// Flip the last character to a different charset symbol.
	corrupt := []byte(enc)
	if corrupt[len(corrupt)-1] == 'q' {
		corrupt[len(corrupt)-1] = 'p'
	} else {
		corrupt[len(corrupt)-1] = 'q'
	}
	if _, _, err := DecodeBech32(string(corrupt)); err == nil {
		t.Fatalf("expected checksum failure on corrupted %q", string(corrupt))
	}
}
