// Package kem implements the hybrid X25519 + ML-KEM-768 key agreement and KEM
// combiner specified in docs/ADR-006-PQC-HYBRID-TRANSPORT.md.
//
// It is the Phase C / C2 conformance + reference harness for Project Aegis. It
// validates, end to end, that the ADR-006 secret-connection handshake derives
// an identical 32-byte session key on both peers, that the wire sizes match the
// ADR's bandwidth table, and that the session key is bound to the handshake
// transcript (so a silent downgrade changes the key).
//
// Runtime crypto is Go 1.24 standard library only — crypto/mlkem, crypto/ecdh,
// crypto/hkdf, crypto/sha256, crypto/rand — with no external dependencies,
// matching the runtime selection in ADR-006. ACVP/KAT conformance of ML-KEM-768
// itself is inherited from the Go standard library's own test suite; this
// harness covers the *composition* (combiner + transcript binding) that ADR-006
// layers on top of it.
package kem

import (
	"crypto/ecdh"
	"crypto/hkdf"
	"crypto/mlkem"
	"crypto/rand"
	"crypto/sha256"
	"crypto/subtle"
	"encoding/binary"
	"errors"
	"fmt"
	"io"
)

// KDFInfo is the ADR-006 domain-separation label for the session-key KDF.
const KDFInfo = "junoclaw/aegis/secret-connection/v1"

// ProtocolVersion is the negotiated handshake tag (ADR-006 §D3). It is bound
// into the transcript so a downgrade cannot be forced silently.
const ProtocolVersion = "secret-connection/v1-hybrid"

// Fixed wire sizes (bytes). The ML-KEM sizes are re-exported from the stdlib so
// that any drift in either the standard library or the ADR fails loudly.
const (
	X25519PubSize    = 32
	MLKEMEncapSize   = mlkem.EncapsulationKeySize768 // 1184
	MLKEMCtSize      = mlkem.CiphertextSize768       // 1088
	SharedSecretSize = mlkem.SharedKeySize           // 32
	SessionKeySize   = 32
)

// Msg1 is the initiator -> responder message. Per ADR-006 §D1 the ML-KEM
// encapsulation key rides alongside the ephemeral X25519 public key.
type Msg1 struct {
	Version   string
	X25519Pub []byte // X25519PubSize
	MLKEMEk   []byte // MLKEMEncapSize
}

// Msg2 is the responder -> initiator message. The ML-KEM ciphertext rides
// alongside the responder's ephemeral X25519 public key.
type Msg2 struct {
	Version   string
	X25519Pub []byte // X25519PubSize
	MLKEMCt   []byte // MLKEMCtSize
}

// InitiatorState holds the ephemeral secrets the initiator must retain between
// sending Msg1 and receiving Msg2. It must not be reused across handshakes.
type InitiatorState struct {
	x25519Priv *ecdh.PrivateKey
	mlkemDecap *mlkem.DecapsulationKey768
}

// InitiatorStart generates the initiator's ephemeral X25519 and ML-KEM-768 keys
// and returns Msg1 plus the state needed to finish the handshake.
func InitiatorStart() (*InitiatorState, *Msg1, error) {
	xPriv, err := ecdh.X25519().GenerateKey(rand.Reader)
	if err != nil {
		return nil, nil, fmt.Errorf("x25519 keygen: %w", err)
	}
	dk, err := mlkem.GenerateKey768()
	if err != nil {
		return nil, nil, fmt.Errorf("ml-kem keygen: %w", err)
	}
	msg1 := &Msg1{
		Version:   ProtocolVersion,
		X25519Pub: xPriv.PublicKey().Bytes(),
		MLKEMEk:   dk.EncapsulationKey().Bytes(),
	}
	return &InitiatorState{x25519Priv: xPriv, mlkemDecap: dk}, msg1, nil
}

// ResponderRespond consumes Msg1, performs the responder side of the hybrid key
// agreement, and returns Msg2 plus the derived 32-byte session key.
func ResponderRespond(msg1 *Msg1) (*Msg2, []byte, error) {
	if msg1.Version != ProtocolVersion {
		return nil, nil, fmt.Errorf("unsupported handshake version %q", msg1.Version)
	}
	if len(msg1.X25519Pub) != X25519PubSize {
		return nil, nil, fmt.Errorf("bad x25519 pub size %d", len(msg1.X25519Pub))
	}
	// Responder ephemeral X25519 + classical shared secret.
	xPriv, err := ecdh.X25519().GenerateKey(rand.Reader)
	if err != nil {
		return nil, nil, fmt.Errorf("x25519 keygen: %w", err)
	}
	iPub, err := ecdh.X25519().NewPublicKey(msg1.X25519Pub)
	if err != nil {
		return nil, nil, fmt.Errorf("parse initiator x25519 pub: %w", err)
	}
	ssClassical, err := xPriv.ECDH(iPub)
	if err != nil {
		return nil, nil, fmt.Errorf("x25519 ecdh: %w", err)
	}
	// Encapsulate against the initiator's ML-KEM encapsulation key.
	ek, err := mlkem.NewEncapsulationKey768(msg1.MLKEMEk)
	if err != nil {
		return nil, nil, fmt.Errorf("parse ml-kem ek: %w", err)
	}
	ssPQ, ct := ek.Encapsulate()

	msg2 := &Msg2{
		Version:   ProtocolVersion,
		X25519Pub: xPriv.PublicKey().Bytes(),
		MLKEMCt:   ct,
	}
	sessionKey, err := deriveSessionKey(ssClassical, ssPQ, transcriptHash(msg1, msg2))
	if err != nil {
		return nil, nil, err
	}
	return msg2, sessionKey, nil
}

// InitiatorFinish consumes Msg2 and returns the initiator's derived session key.
// The msg1 passed here is the transcript the initiator authenticates; binding
// it (rather than trusting the responder's echo) is what makes a silent
// downgrade detectable — see TestTranscriptBindingDowngradeDetected.
func (st *InitiatorState) InitiatorFinish(msg1 *Msg1, msg2 *Msg2) ([]byte, error) {
	if msg2.Version != ProtocolVersion {
		return nil, fmt.Errorf("unsupported handshake version %q", msg2.Version)
	}
	if len(msg2.X25519Pub) != X25519PubSize {
		return nil, fmt.Errorf("bad x25519 pub size %d", len(msg2.X25519Pub))
	}
	rPub, err := ecdh.X25519().NewPublicKey(msg2.X25519Pub)
	if err != nil {
		return nil, fmt.Errorf("parse responder x25519 pub: %w", err)
	}
	ssClassical, err := st.x25519Priv.ECDH(rPub)
	if err != nil {
		return nil, fmt.Errorf("x25519 ecdh: %w", err)
	}
	ssPQ, err := st.mlkemDecap.Decapsulate(msg2.MLKEMCt)
	if err != nil {
		return nil, fmt.Errorf("ml-kem decapsulate: %w", err)
	}
	return deriveSessionKey(ssClassical, ssPQ, transcriptHash(msg1, msg2))
}

// HybridHandshake runs a full in-process initiator/responder exchange and
// returns both derived session keys plus the two wire messages (for size
// inspection). In a real deployment Msg1/Msg2 cross the network.
func HybridHandshake() (initiatorKey, responderKey []byte, msg1 *Msg1, msg2 *Msg2, err error) {
	st, m1, err := InitiatorStart()
	if err != nil {
		return nil, nil, nil, nil, err
	}
	m2, rKey, err := ResponderRespond(m1)
	if err != nil {
		return nil, nil, nil, nil, err
	}
	iKey, err := st.InitiatorFinish(m1, m2)
	if err != nil {
		return nil, nil, nil, nil, err
	}
	return iKey, rKey, m1, m2, nil
}

// deriveSessionKey implements the ADR-006 KEM combiner:
//
//	session_key = HKDF-SHA256(
//	    ikm  = ss_classical || ss_pq,           // 64 B, fixed-size halves
//	    salt = H(transcript),
//	    info = "junoclaw/aegis/secret-connection/v1")
//
// IND-CCA of the combined key holds if either component KEM is secure.
func deriveSessionKey(ssClassical, ssPQ, salt []byte) ([]byte, error) {
	if len(ssClassical) != SharedSecretSize || len(ssPQ) != SharedSecretSize {
		return nil, fmt.Errorf("unexpected shared-secret size: classical=%d pq=%d", len(ssClassical), len(ssPQ))
	}
	ikm := make([]byte, 0, 2*SharedSecretSize)
	ikm = append(ikm, ssClassical...)
	ikm = append(ikm, ssPQ...)
	return hkdf.Key(sha256.New, ikm, salt, KDFInfo, SessionKeySize)
}

// transcriptHash binds the negotiated version and every exchanged public value
// (domain-separated, length-prefixed) into the KDF salt. Any tampering — most
// importantly a forced version downgrade — changes the salt and therefore the
// session key, so the two peers fail to agree and the channel never forms.
func transcriptHash(msg1 *Msg1, msg2 *Msg2) []byte {
	h := sha256.New()
	writeField(h, []byte("ADR-006/transcript"))
	writeField(h, []byte(msg1.Version))
	writeField(h, msg1.X25519Pub)
	writeField(h, msg1.MLKEMEk)
	writeField(h, []byte(msg2.Version))
	writeField(h, msg2.X25519Pub)
	writeField(h, msg2.MLKEMCt)
	return h.Sum(nil)
}

// writeField appends a 4-byte big-endian length prefix followed by the field
// bytes, so distinct field boundaries can never be confused (no canonical
// ambiguity in the transcript).
func writeField(h io.Writer, b []byte) {
	var l [4]byte
	binary.BigEndian.PutUint32(l[:], uint32(len(b)))
	_, _ = h.Write(l[:])
	_, _ = h.Write(b)
}

// Equal reports whether two derived keys are identical, in constant time.
func Equal(a, b []byte) bool {
	if len(a) != len(b) {
		return false
	}
	return subtle.ConstantTimeCompare(a, b) == 1
}

// ErrSizeMismatch is returned by SelfCheckSizes when a constant drifts.
var ErrSizeMismatch = errors.New("ML-KEM-768 size mismatch vs ADR-006")

// SelfCheckSizes asserts the compiled-in sizes match the ADR-006 table. It is
// used by both the demo binary and the tests as a cheap drift detector.
func SelfCheckSizes() error {
	if MLKEMEncapSize != 1184 || MLKEMCtSize != 1088 || SharedSecretSize != 32 {
		return fmt.Errorf("%w: ek=%d ct=%d ss=%d", ErrSizeMismatch, MLKEMEncapSize, MLKEMCtSize, SharedSecretSize)
	}
	return nil
}
