package kem

import (
	"bytes"
	"crypto/mlkem"
	"testing"
)

// TestHybridHandshakeAgreement is the core property: initiator and responder
// independently derive the same 32-byte session key.
func TestHybridHandshakeAgreement(t *testing.T) {
	iKey, rKey, _, _, err := HybridHandshake()
	if err != nil {
		t.Fatalf("handshake: %v", err)
	}
	if !Equal(iKey, rKey) {
		t.Fatalf("session keys differ:\n i=%x\n r=%x", iKey, rKey)
	}
	if len(iKey) != SessionKeySize {
		t.Fatalf("session key len = %d, want %d", len(iKey), SessionKeySize)
	}
}

// TestWireSizesMatchADR006 pins the on-wire sizes to the ADR-006 bandwidth
// table, so any stdlib or constant drift fails loudly.
func TestWireSizesMatchADR006(t *testing.T) {
	if err := SelfCheckSizes(); err != nil {
		t.Fatal(err)
	}
	_, _, m1, m2, err := HybridHandshake()
	if err != nil {
		t.Fatalf("handshake: %v", err)
	}
	cases := []struct {
		name string
		got  int
		want int
	}{
		{"Msg1 X25519 pub", len(m1.X25519Pub), X25519PubSize},
		{"Msg1 ML-KEM ek", len(m1.MLKEMEk), 1184},
		{"Msg2 X25519 pub", len(m2.X25519Pub), X25519PubSize},
		{"Msg2 ML-KEM ct", len(m2.MLKEMCt), 1088},
	}
	for _, c := range cases {
		if c.got != c.want {
			t.Errorf("%s size = %d, want %d", c.name, c.got, c.want)
		}
	}
}

// TestTranscriptBindingDowngradeDetected proves the session key is bound to the
// negotiated version: if an attacker flips the version the initiator
// authenticates (a forced downgrade), the two peers no longer agree.
func TestTranscriptBindingDowngradeDetected(t *testing.T) {
	st, m1, err := InitiatorStart()
	if err != nil {
		t.Fatalf("initiator start: %v", err)
	}
	m2, rKey, err := ResponderRespond(m1)
	if err != nil {
		t.Fatalf("responder: %v", err)
	}
	// Same secrets, but the initiator authenticates a tampered transcript.
	tampered := *m1
	tampered.Version = "secret-connection/v0-legacy"
	iKey, err := st.InitiatorFinish(&tampered, m2)
	if err != nil {
		t.Fatalf("initiator finish: %v", err)
	}
	if Equal(iKey, rKey) {
		t.Fatal("tampered transcript still agreed — transcript binding is broken")
	}
}

// TestCiphertextTamperBreaksAgreement confirms a flipped ML-KEM ciphertext
// yields a different shared secret (FIPS 203 implicit rejection) rather than a
// panic, and that the peers therefore fail to agree.
func TestCiphertextTamperBreaksAgreement(t *testing.T) {
	st, m1, err := InitiatorStart()
	if err != nil {
		t.Fatalf("initiator start: %v", err)
	}
	m2, rKey, err := ResponderRespond(m1)
	if err != nil {
		t.Fatalf("responder: %v", err)
	}
	m2.MLKEMCt[0] ^= 0xFF // tamper
	iKey, err := st.InitiatorFinish(m1, m2)
	if err != nil {
		t.Fatalf("initiator finish should not error on tampered ct (implicit rejection): %v", err)
	}
	if Equal(iKey, rKey) {
		t.Fatal("tampered ciphertext still agreed")
	}
}

// TestDeterministicCombiner checks deriveSessionKey is a pure, order-sensitive
// function of its inputs (the combiner must not be commutative in its halves).
func TestDeterministicCombiner(t *testing.T) {
	ssC := bytes.Repeat([]byte{0x11}, SharedSecretSize)
	ssQ := bytes.Repeat([]byte{0x22}, SharedSecretSize)
	salt := bytes.Repeat([]byte{0x33}, 32)

	k1, err := deriveSessionKey(ssC, ssQ, salt)
	if err != nil {
		t.Fatalf("derive: %v", err)
	}
	k2, err := deriveSessionKey(ssC, ssQ, salt)
	if err != nil {
		t.Fatalf("derive: %v", err)
	}
	if !bytes.Equal(k1, k2) {
		t.Fatal("combiner is not deterministic")
	}
	k3, err := deriveSessionKey(ssQ, ssC, salt) // swapped halves
	if err != nil {
		t.Fatalf("derive: %v", err)
	}
	if bytes.Equal(k1, k3) {
		t.Fatal("combiner ignored the ordering of its inputs")
	}
}

// TestWireRoundTrip confirms Msg1/Msg2 survive marshal→unmarshal unchanged and
// that the decoded messages still derive an agreeing session key (i.e. the wire
// codec preserves exactly the bytes the transcript authenticates).
func TestWireRoundTrip(t *testing.T) {
	st, m1, err := InitiatorStart()
	if err != nil {
		t.Fatalf("initiator start: %v", err)
	}
	m2, rKey, err := ResponderRespond(m1)
	if err != nil {
		t.Fatalf("responder: %v", err)
	}

	b1, err := m1.MarshalBinary()
	if err != nil {
		t.Fatalf("marshal msg1: %v", err)
	}
	b2, err := m2.MarshalBinary()
	if err != nil {
		t.Fatalf("marshal msg2: %v", err)
	}
	d1, err := UnmarshalMsg1(b1)
	if err != nil {
		t.Fatalf("unmarshal msg1: %v", err)
	}
	d2, err := UnmarshalMsg2(b2)
	if err != nil {
		t.Fatalf("unmarshal msg2: %v", err)
	}
	if d1.Version != m1.Version || !bytes.Equal(d1.X25519Pub, m1.X25519Pub) || !bytes.Equal(d1.MLKEMEk, m1.MLKEMEk) {
		t.Fatal("msg1 changed across wire codec")
	}
	if d2.Version != m2.Version || !bytes.Equal(d2.X25519Pub, m2.X25519Pub) || !bytes.Equal(d2.MLKEMCt, m2.MLKEMCt) {
		t.Fatal("msg2 changed across wire codec")
	}
	// The decoded messages must still produce the responder's session key.
	iKey, err := st.InitiatorFinish(d1, d2)
	if err != nil {
		t.Fatalf("initiator finish on decoded msgs: %v", err)
	}
	if !Equal(iKey, rKey) {
		t.Fatal("session keys differ after wire round-trip")
	}
}

// TestFrameRoundTrip confirms WriteFrame/ReadFrame preserve payloads and report
// matching byte counts.
func TestFrameRoundTrip(t *testing.T) {
	payload := bytes.Repeat([]byte{0xAB}, 1184)
	var buf bytes.Buffer
	wrote, err := WriteFrame(&buf, payload)
	if err != nil {
		t.Fatalf("write frame: %v", err)
	}
	got, read, err := ReadFrame(&buf)
	if err != nil {
		t.Fatalf("read frame: %v", err)
	}
	if wrote != read || wrote != 4+len(payload) {
		t.Fatalf("frame byte counts mismatch: wrote=%d read=%d want=%d", wrote, read, 4+len(payload))
	}
	if !bytes.Equal(got, payload) {
		t.Fatal("frame payload changed")
	}
}

// TestMLKEMRoundTrip is a sanity round-trip of the stdlib ML-KEM-768 itself.
// (Full ACVP/KAT conformance is inherited from the Go standard library's own
// test suite; this only confirms the primitive is wired correctly here.)
func TestMLKEMRoundTrip(t *testing.T) {
	dk, err := mlkem.GenerateKey768()
	if err != nil {
		t.Fatalf("keygen: %v", err)
	}
	ek := dk.EncapsulationKey()
	ssA, ct := ek.Encapsulate()
	ssB, err := dk.Decapsulate(ct)
	if err != nil {
		t.Fatalf("decapsulate: %v", err)
	}
	if !bytes.Equal(ssA, ssB) {
		t.Fatal("ML-KEM-768 round-trip shared secrets differ")
	}
	if len(ssA) != SharedSecretSize {
		t.Fatalf("shared secret len = %d, want %d", len(ssA), SharedSecretSize)
	}
}
