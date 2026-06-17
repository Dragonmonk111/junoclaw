package secretconn

import (
	"bytes"
	"crypto/ed25519"
	"crypto/rand"
	"errors"
	"io"
	"net"
	"testing"

	"github.com/junoclaw/aegis-transport/kem"
)

// genKey returns a fresh Ed25519 node key.
func genKey(t *testing.T) ed25519.PrivateKey {
	t.Helper()
	_, priv, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatalf("ed25519 keygen: %v", err)
	}
	return priv
}

type peerResult struct {
	sc  *SecretConn
	err error
}

// handshakePair runs MakeSecretConnection on both ends of conns a and b
// concurrently and returns both results.
func handshakePair(a, b io.ReadWriteCloser, ka, kb ed25519.PrivateKey) (peerResult, peerResult) {
	ca := make(chan peerResult, 1)
	cb := make(chan peerResult, 1)
	go func() { sc, err := MakeSecretConnection(a, ka); ca <- peerResult{sc, err} }()
	go func() { sc, err := MakeSecretConnection(b, kb); cb <- peerResult{sc, err} }()
	return <-ca, <-cb
}

func TestSecretConnAgreementOverPipe(t *testing.T) {
	a, b := net.Pipe()
	defer a.Close()
	defer b.Close()
	ka, kb := genKey(t), genKey(t)

	ra, rb := handshakePair(a, b, ka, kb)
	if ra.err != nil || rb.err != nil {
		t.Fatalf("handshake failed: a=%v b=%v", ra.err, rb.err)
	}

	// Each side must have authenticated the other's node pubkey.
	if !bytes.Equal(ra.sc.RemotePubKey(), kb.Public().(ed25519.PublicKey)) {
		t.Errorf("peer A did not authenticate B's pubkey")
	}
	if !bytes.Equal(rb.sc.RemotePubKey(), ka.Public().(ed25519.PublicKey)) {
		t.Errorf("peer B did not authenticate A's pubkey")
	}

	// Bidirectional encrypted round-trip.
	want := []byte("post-quantum hello over the wire")
	done := make(chan error, 1)
	go func() {
		buf := make([]byte, len(want))
		if _, err := io.ReadFull(rb.sc, buf); err != nil {
			done <- err
			return
		}
		if !bytes.Equal(buf, want) {
			done <- errors.New("payload mismatch B<-A")
			return
		}
		done <- rb.sc.Write([]byte("ack"))
	}()
	if err := ra.sc.Write(want); err != nil {
		t.Fatalf("A write: %v", err)
	}
	// Read the ack concurrently with B writing it: on an unbuffered pipe B's
	// Write blocks until A reads, so A must not wait on <-done first.
	ack := make([]byte, 3)
	if _, err := io.ReadFull(ra.sc, ack); err != nil {
		t.Fatalf("A read ack: %v", err)
	}
	if err := <-done; err != nil {
		t.Fatalf("B side: %v", err)
	}
	if string(ack) != "ack" {
		t.Errorf("ack mismatch: %q", ack)
	}
}

func TestSecretConnOverTCP(t *testing.T) {
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("listen: %v", err)
	}
	defer ln.Close()
	ka, kb := genKey(t), genKey(t)

	srv := make(chan peerResult, 1)
	go func() {
		c, err := ln.Accept()
		if err != nil {
			srv <- peerResult{nil, err}
			return
		}
		sc, err := MakeSecretConnection(c, kb)
		srv <- peerResult{sc, err}
	}()

	c, err := net.Dial("tcp", ln.Addr().String())
	if err != nil {
		t.Fatalf("dial: %v", err)
	}
	defer c.Close()
	csc, err := MakeSecretConnection(c, ka)
	if err != nil {
		t.Fatalf("client handshake: %v", err)
	}
	s := <-srv
	if s.err != nil {
		t.Fatalf("server handshake: %v", s.err)
	}

	if !bytes.Equal(csc.RemotePubKey(), kb.Public().(ed25519.PublicKey)) {
		t.Errorf("client did not authenticate server pubkey")
	}
}

func TestTamperedFrameRejected(t *testing.T) {
	a, b := net.Pipe()
	defer a.Close()
	defer b.Close()
	ra, rb := handshakePair(a, b, genKey(t), genKey(t))
	if ra.err != nil || rb.err != nil {
		t.Fatalf("handshake failed: a=%v b=%v", ra.err, rb.err)
	}

	// A seals a frame; we corrupt it in flight before B opens it. Because we
	// can't intercept inside SecretConn.Write here, assert the AEAD directly:
	// flipping any ciphertext byte must make Open fail.
	ct := ra.sc.send.Seal(nil, nonce(0), []byte("secret"), nil)
	ct[0] ^= 0xff
	if _, err := rb.sc.recv.Open(nil, nonce(0), ct, nil); err == nil {
		t.Fatal("tampered ciphertext opened without error")
	}
}

// badVersionPeer plays one side of the handshake but advertises a wrong version
// tag, simulating a downgrade attempt. The honest peer must reject it.
func badVersionPeer(conn io.ReadWriteCloser) {
	// The honest peer checks the version before parsing the pubkey, so any
	// well-formed-length bytes suffice here.
	hello := &ephHello{
		Version:   "secret-connection/v0-classical",
		X25519Pub: make([]byte, kem.X25519PubSize),
		MLKEMEk:   make([]byte, kem.MLKEMEncapSize),
	}
	_, _ = exchangeRaw(conn, hello.marshal())
}

func TestWrongVersionRejected(t *testing.T) {
	a, b := net.Pipe()
	defer a.Close()
	defer b.Close()

	go badVersionPeer(b)
	_, err := MakeSecretConnection(a, genKey(t))
	if !errors.Is(err, ErrDowngrade) {
		t.Fatalf("expected ErrDowngrade, got %v", err)
	}
}

func TestTranscriptBindingChangesKeys(t *testing.T) {
	// The ADR-006 transcript-binding property: any change to an exchanged
	// public value (which a MITM would have to make) changes the KDF salt and
	// therefore every derived key, so the two peers cannot agree and mutual
	// auth — which signs over a key derived from the transcript — fails.
	ss1 := bytes.Repeat([]byte{0x11}, kem.SharedSecretSize)
	ss2 := bytes.Repeat([]byte{0x22}, kem.SharedSecretSize)

	loEph := bytes.Repeat([]byte{0xa1}, kem.X25519PubSize)
	hiEph := bytes.Repeat([]byte{0xb2}, kem.X25519PubSize)
	loEk := bytes.Repeat([]byte{0xc3}, kem.MLKEMEncapSize)
	ct := bytes.Repeat([]byte{0xd4}, kem.MLKEMCtSize)

	base := handshakeTranscript(loEph, hiEph, loEk, ct)
	kmBase, err := deriveKeyMaterial(ss1, ss2, base)
	if err != nil {
		t.Fatalf("deriveKeyMaterial: %v", err)
	}

	// Same inputs -> identical key material (determinism).
	kmSame, _ := deriveKeyMaterial(ss1, ss2, handshakeTranscript(loEph, hiEph, loEk, ct))
	if !bytes.Equal(kmBase, kmSame) {
		t.Fatal("identical transcript produced different key material")
	}

	// A single flipped byte in the ML-KEM ek (a tampered hello) must change all
	// derived key material.
	tamperedEk := append([]byte(nil), loEk...)
	tamperedEk[0] ^= 0xff
	kmTamper, _ := deriveKeyMaterial(ss1, ss2, handshakeTranscript(loEph, hiEph, tamperedEk, ct))
	if bytes.Equal(kmBase, kmTamper) {
		t.Fatal("tampered transcript produced identical key material (binding broken)")
	}
}
