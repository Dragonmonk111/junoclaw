// Package secretconn is the Project Aegis Phase C "fold into CometBFT" artifact:
// a CometBFT-shaped, drop-in hybrid secret connection that wraps an arbitrary
// io.ReadWriteCloser in an authenticated, encrypted channel whose key agreement
// is the ADR-006 hybrid X25519 + ML-KEM-768 construction.
//
// It deliberately mirrors the structure of CometBFT's
// p2p/conn/secret_connection.go so that it can be applied as a fork patch with
// minimal translation (see PORTING.md):
//
//	stock CometBFT                         this package
//	--------------------------------       ---------------------------------
//	MakeSecretConnection(conn, privKey)     MakeSecretConnection(conn, privKey)
//	genEphKeys()                            genEphKeys()  (+ ML-KEM ek/decap)
//	shareEphPubKey()                        shareEphHello()  (eph pub + ML-KEM ek)
//	deriveSecrets()                         deriveSecrets()  (ss_classical||ss_pq)
//	shareAuthSignature()                    shareAuthSignature()
//	(Read / Write AEAD frames)              (Read / Write AEAD frames)
//
// Two intentional, documented divergences from stock CometBFT keep this harness
// dependency-free (Go 1.24 standard library only, matching ADR-006's runtime
// selection and the rest of aegis-transport):
//
//   - AEAD: AES-256-GCM (crypto/aes + crypto/cipher) instead of CometBFT's
//     ChaCha20-Poly1305 (golang.org/x/crypto). A real fork keeps ChaCha20 to
//     match CometBFT's existing frame format; the combiner/auth changes are
//     orthogonal to the AEAD choice. See PORTING.md.
//   - Node auth: Ed25519 (crypto/ed25519). ADR-006 §D2's production target is a
//     HYBRID Ed25519 + ML-DSA-44 node key; the ML-DSA half plugs in at the
//     clearly-marked extension point in shareAuthSignature (reusing the same
//     vendored ML-DSA-44 as the rest of Aegis — not a second impl).
//
// The hybrid KEM combiner, transcript binding, and downgrade resistance — the
// security-critical parts ADR-006 specifies — are implemented exactly, reusing
// the verified kem package.
package secretconn

import (
	"bytes"
	"crypto/aes"
	"crypto/cipher"
	"crypto/ecdh"
	"crypto/ed25519"
	"crypto/hkdf"
	"crypto/mlkem"
	"crypto/rand"
	"crypto/sha256"
	"encoding/binary"
	"errors"
	"fmt"
	"io"

	"github.com/junoclaw/aegis-transport/kem"
)

// Wire/identity constants. The protocol version is bound into the transcript so
// a silent downgrade (e.g. to a classical-only build) changes the derived keys
// and the handshake fails closed.
const (
	// ProtocolVersion is re-exported from kem so the secret connection and the
	// bare combiner can never disagree on the negotiated tag.
	ProtocolVersion = kem.ProtocolVersion

	// authKDFInfo derives the directional AEAD keys and the auth challenge.
	authKDFInfo = "junoclaw/aegis/secret-connection/keys/v1"

	keyLen       = 32 // AES-256-GCM key
	challengeLen = 32

	// maxFrameLen caps a single sealed application frame's plaintext, bounding
	// the read buffer a peer will allocate (anti-DoS), mirroring CometBFT's
	// fixed-chunk discipline in spirit.
	maxFrameLen = 1 << 20 // 1 MiB
)

var (
	// ErrDowngrade is returned when the peer advertises a different handshake
	// version than ours (negotiation is fail-closed in this harness; the real
	// fork falls back to legacy per ADR-006 §D3 and logs a non-PQ peer).
	ErrDowngrade = errors.New("secretconn: handshake version mismatch (possible downgrade)")
	// ErrAuth is returned when the peer's node-key signature over the handshake
	// challenge does not verify.
	ErrAuth = errors.New("secretconn: peer authentication failed")
	// ErrFrameTooLarge guards the read path against an oversized length prefix.
	ErrFrameTooLarge = errors.New("secretconn: frame exceeds maximum length")
)

// ephHello is the first handshake message: the ephemeral X25519 public key and
// the ML-KEM-768 encapsulation key, exchanged simultaneously by both peers
// (CometBFT exchanges only the eph pubkey here; ADR-006 rides ek alongside).
type ephHello struct {
	Version   string
	X25519Pub []byte // kem.X25519PubSize
	MLKEMEk   []byte // kem.MLKEMEncapSize
}

func (e *ephHello) marshal() []byte {
	var buf bytes.Buffer
	writeField(&buf, []byte(e.Version))
	writeField(&buf, e.X25519Pub)
	writeField(&buf, e.MLKEMEk)
	return buf.Bytes()
}

func unmarshalEphHello(b []byte) (*ephHello, error) {
	r := bytes.NewReader(b)
	ver, err := readField(r)
	if err != nil {
		return nil, fmt.Errorf("ephHello version: %w", err)
	}
	xpub, err := readField(r)
	if err != nil {
		return nil, fmt.Errorf("ephHello x25519: %w", err)
	}
	ek, err := readField(r)
	if err != nil {
		return nil, fmt.Errorf("ephHello ek: %w", err)
	}
	if r.Len() != 0 {
		return nil, fmt.Errorf("ephHello has %d trailing bytes", r.Len())
	}
	return &ephHello{Version: string(ver), X25519Pub: xpub, MLKEMEk: ek}, nil
}

// SecretConn is an authenticated, encrypted, post-quantum-hybrid connection. It
// satisfies net.Conn's Read/Write/Close subset (the part CometBFT's p2p layer
// uses); embed in a net.Conn wrapper for the full interface in the real fork.
type SecretConn struct {
	conn io.ReadWriteCloser

	send     cipher.AEAD
	recv     cipher.AEAD
	sendNctr uint64
	recvNctr uint64

	remotePubKey ed25519.PublicKey

	readBuf bytes.Buffer // leftover plaintext from a partially consumed frame
}

// RemotePubKey returns the authenticated Ed25519 node public key of the peer.
// In the production hybrid build this becomes the hybrid (Ed25519, ML-DSA-44)
// identity.
func (sc *SecretConn) RemotePubKey() ed25519.PublicKey { return sc.remotePubKey }

// MakeSecretConnection performs the ADR-006 hybrid handshake over conn and
// returns an encrypted SecretConn. localPrivKey is the persistent node key used
// to authenticate this peer to the other side (CometBFT's NodeKey.PrivKey).
//
// The flow mirrors CometBFT: (1) exchange ephemeral material, (2) derive shared
// secrets, (3) exchange and verify auth signatures over the transcript-derived
// challenge. The post-quantum additions are folded into (1) and (2).
func MakeSecretConnection(conn io.ReadWriteCloser, localPrivKey ed25519.PrivateKey) (*SecretConn, error) {
	// (1) Ephemeral key generation: X25519 + ML-KEM-768, per ADR-006 §D1.
	ephPriv, err := ecdh.X25519().GenerateKey(rand.Reader)
	if err != nil {
		return nil, fmt.Errorf("x25519 keygen: %w", err)
	}
	mlkemDecap, err := mlkem.GenerateKey768()
	if err != nil {
		return nil, fmt.Errorf("ml-kem keygen: %w", err)
	}

	localHello := &ephHello{
		Version:   ProtocolVersion,
		X25519Pub: ephPriv.PublicKey().Bytes(),
		MLKEMEk:   mlkemDecap.EncapsulationKey().Bytes(),
	}

	// Simultaneous exchange of ephemeral hellos. Both peers send first, so the
	// read is done concurrently with the write to avoid a deadlock on an
	// unbuffered duplex stream (e.g. net.Pipe). Mirrors CometBFT, which runs the
	// eph-pubkey exchange in parallel.
	remoteBytes, err := exchangeRaw(conn, localHello.marshal())
	if err != nil {
		return nil, fmt.Errorf("eph hello exchange: %w", err)
	}
	remoteHello, err := unmarshalEphHello(remoteBytes)
	if err != nil {
		return nil, err
	}
	if remoteHello.Version != ProtocolVersion {
		return nil, fmt.Errorf("%w: local %q remote %q", ErrDowngrade, ProtocolVersion, remoteHello.Version)
	}
	if len(remoteHello.X25519Pub) != kem.X25519PubSize {
		return nil, fmt.Errorf("bad remote x25519 pub size %d", len(remoteHello.X25519Pub))
	}

	// Deterministic role assignment from the sorted ephemeral pubkeys, exactly
	// as CometBFT derives loEphPub/hiEphPub. The "lo" peer is the ML-KEM
	// initiator (its ek is encapsulated against); the "hi" peer is the ML-KEM
	// responder (it encapsulates and sends the ciphertext). This keeps a single
	// ek/ct pair on the wire as ADR-006 specifies, rather than a wasteful
	// bidirectional KEM.
	loEph, hiEph, weAreLo := orderEph(localHello.X25519Pub, remoteHello.X25519Pub)

	// (2a) Classical shared secret (X25519 ECDH).
	remoteEphPub, err := ecdh.X25519().NewPublicKey(remoteHello.X25519Pub)
	if err != nil {
		return nil, fmt.Errorf("parse remote x25519 pub: %w", err)
	}
	ssClassical, err := ephPriv.ECDH(remoteEphPub)
	if err != nil {
		return nil, fmt.Errorf("x25519 ecdh: %w", err)
	}

	// (2b) Post-quantum shared secret (ML-KEM-768). The lo peer's ek is the one
	// used; the hi peer encapsulates against it and transmits ct.
	var ssPQ, ct, loEk []byte
	if weAreLo {
		loEk = localHello.MLKEMEk
		ctBytes, _, rerr := kem.ReadFrame(conn) // hi -> lo
		if rerr != nil {
			return nil, fmt.Errorf("read ml-kem ct: %w", rerr)
		}
		ct = ctBytes
		ssPQ, err = mlkemDecap.Decapsulate(ct)
		if err != nil {
			return nil, fmt.Errorf("ml-kem decapsulate: %w", err)
		}
	} else {
		loEk = remoteHello.MLKEMEk
		ek, eerr := mlkem.NewEncapsulationKey768(loEk)
		if eerr != nil {
			return nil, fmt.Errorf("parse lo ml-kem ek: %w", eerr)
		}
		ssPQ, ct = ek.Encapsulate()
		if _, werr := kem.WriteFrame(conn, ct); werr != nil { // hi -> lo
			return nil, fmt.Errorf("write ml-kem ct: %w", werr)
		}
	}

	// (2c) Transcript binding + key derivation. The transcript covers the
	// version and every exchanged public value in a canonical (lo, hi) order so
	// both peers compute the same salt regardless of who dialed.
	transcript := handshakeTranscript(loEph, hiEph, loEk, ct)
	km, err := deriveKeyMaterial(ssClassical, ssPQ, transcript)
	if err != nil {
		return nil, err
	}
	loSendKey := km[0:keyLen]
	hiSendKey := km[keyLen : 2*keyLen]
	challenge := km[2*keyLen : 2*keyLen+challengeLen]

	var sendKey, recvKey []byte
	if weAreLo {
		sendKey, recvKey = loSendKey, hiSendKey
	} else {
		sendKey, recvKey = hiSendKey, loSendKey
	}

	sc := &SecretConn{conn: conn}
	if sc.send, err = newAEAD(sendKey); err != nil {
		return nil, err
	}
	if sc.recv, err = newAEAD(recvKey); err != nil {
		return nil, err
	}

	// (3) Mutual authentication: sign the challenge with the node key, exchange
	// the signatures *inside the now-encrypted channel*, and verify. This binds
	// the ephemeral hybrid exchange to a long-lived identity (station-to-station)
	// and makes a silent downgrade require forging the signature too.
	if err := sc.shareAuthSignature(localPrivKey, challenge); err != nil {
		return nil, err
	}
	return sc, nil
}

// orderEph returns the lexicographically smaller and larger ephemeral pubkeys
// and whether ours is the smaller ("lo"). Mirrors CometBFT's loEphPub/hiEphPub.
func orderEph(localPub, remotePub []byte) (lo, hi []byte, weAreLo bool) {
	if bytes.Compare(localPub, remotePub) < 0 {
		return localPub, remotePub, true
	}
	return remotePub, localPub, false
}

// handshakeTranscript binds the negotiated version and every exchanged public
// value (domain-separated, length-prefixed, canonical lo/hi order) into the KDF
// salt — the ADR-006 transcript-binding / downgrade-resistance mechanism.
func handshakeTranscript(loEph, hiEph, loEk, ct []byte) []byte {
	h := sha256.New()
	writeField(h, []byte("ADR-006/secret-connection/transcript"))
	writeField(h, []byte(ProtocolVersion))
	writeField(h, loEph)
	writeField(h, hiEph)
	writeField(h, loEk)
	writeField(h, ct)
	return h.Sum(nil)
}

// deriveKeyMaterial implements the ADR-006 KEM combiner and expands it into two
// directional AEAD keys plus the auth challenge:
//
//	ikm = ss_classical || ss_pq            (64 B, fixed-size halves)
//	km  = HKDF-SHA256(ikm, salt=transcript, info, 2*keyLen+challengeLen)
//
// IND-CCA of the combined key holds if either component KEM is secure.
func deriveKeyMaterial(ssClassical, ssPQ, transcript []byte) ([]byte, error) {
	if len(ssClassical) != kem.SharedSecretSize || len(ssPQ) != kem.SharedSecretSize {
		return nil, fmt.Errorf("unexpected shared-secret size: classical=%d pq=%d", len(ssClassical), len(ssPQ))
	}
	ikm := make([]byte, 0, 2*kem.SharedSecretSize)
	ikm = append(ikm, ssClassical...)
	ikm = append(ikm, ssPQ...)
	return hkdf.Key(sha256.New, ikm, transcript, authKDFInfo, 2*keyLen+challengeLen)
}

// authMsg carries a peer's node public key and its signature over the
// handshake challenge. In the hybrid production build PubKey/Sig become the
// tagged hybrid (Ed25519, ML-DSA-44) key and concatenated signatures.
type authMsg struct {
	PubKey []byte
	Sig    []byte
}

func (a *authMsg) marshal() []byte {
	var buf bytes.Buffer
	writeField(&buf, a.PubKey)
	writeField(&buf, a.Sig)
	return buf.Bytes()
}

func unmarshalAuthMsg(b []byte) (*authMsg, error) {
	r := bytes.NewReader(b)
	pk, err := readField(r)
	if err != nil {
		return nil, fmt.Errorf("authMsg pubkey: %w", err)
	}
	sig, err := readField(r)
	if err != nil {
		return nil, fmt.Errorf("authMsg sig: %w", err)
	}
	if r.Len() != 0 {
		return nil, fmt.Errorf("authMsg has %d trailing bytes", r.Len())
	}
	return &authMsg{PubKey: pk, Sig: sig}, nil
}

// shareAuthSignature signs the challenge, exchanges the auth message over the
// encrypted channel, and verifies the peer's signature.
//
// EXTENSION POINT (ADR-006 §D2 hybrid auth): to go hybrid, sign the challenge
// with BOTH the Ed25519 node key and the vendored ML-DSA-44 key, concatenate
// the signatures (algorithm-tagged), and require BOTH to verify here. The
// vendored ML-DSA-44 is the same impl used by consensus/accounts — do not
// introduce a second one. Everything else in this function is unchanged.
func (sc *SecretConn) shareAuthSignature(privKey ed25519.PrivateKey, challenge []byte) error {
	sig := ed25519.Sign(privKey, challenge)
	pub := privKey.Public().(ed25519.PublicKey)
	local := &authMsg{PubKey: pub, Sig: sig}

	// Concurrent send/receive over the encrypted channel: send and recv use
	// independent keys and nonce counters, so the two directions are safe to run
	// in parallel and this avoids a deadlock when both peers send first.
	type result struct {
		b   []byte
		err error
	}
	rc := make(chan result, 1)
	go func() {
		b, err := sc.readFrame()
		rc <- result{b, err}
	}()
	if err := sc.Write(local.marshal()); err != nil {
		return fmt.Errorf("write auth: %w", err)
	}
	res := <-rc
	if res.err != nil {
		return fmt.Errorf("read auth: %w", res.err)
	}
	remote, err := unmarshalAuthMsg(res.b)
	if err != nil {
		return err
	}
	if len(remote.PubKey) != ed25519.PublicKeySize {
		return fmt.Errorf("%w: bad pubkey size %d", ErrAuth, len(remote.PubKey))
	}
	if !ed25519.Verify(ed25519.PublicKey(remote.PubKey), challenge, remote.Sig) {
		return ErrAuth
	}
	sc.remotePubKey = ed25519.PublicKey(remote.PubKey)
	return nil
}

// ---- AEAD framing ----------------------------------------------------------

func newAEAD(key []byte) (cipher.AEAD, error) {
	block, err := aes.NewCipher(key)
	if err != nil {
		return nil, fmt.Errorf("aes cipher: %w", err)
	}
	return cipher.NewGCM(block)
}

// nonce builds a 12-byte GCM nonce from a monotonically increasing counter. The
// send and recv directions use independent keys and independent counters, so a
// (key, nonce) pair is never reused within a connection.
func nonce(ctr uint64) []byte {
	var n [12]byte
	binary.BigEndian.PutUint64(n[4:], ctr)
	return n[:]
}

// Write seals p as a single length-prefixed AEAD frame and writes it.
func (sc *SecretConn) Write(p []byte) error {
	if len(p) > maxFrameLen {
		return ErrFrameTooLarge
	}
	ct := sc.send.Seal(nil, nonce(sc.sendNctr), p, nil)
	sc.sendNctr++
	if _, err := kem.WriteFrame(sc.conn, ct); err != nil {
		return err
	}
	return nil
}

// readFrame reads one sealed frame and returns the opened plaintext.
func (sc *SecretConn) readFrame() ([]byte, error) {
	ct, _, err := kem.ReadFrame(sc.conn)
	if err != nil {
		return nil, err
	}
	if len(ct) > maxFrameLen+sc.recv.Overhead() {
		return nil, ErrFrameTooLarge
	}
	pt, err := sc.recv.Open(nil, nonce(sc.recvNctr), ct, nil)
	if err != nil {
		return nil, fmt.Errorf("secretconn: decrypt: %w", err)
	}
	sc.recvNctr++
	return pt, nil
}

// Read implements io.Reader over the decrypted stream, buffering any plaintext
// not consumed by the caller's buffer.
func (sc *SecretConn) Read(p []byte) (int, error) {
	if sc.readBuf.Len() == 0 {
		pt, err := sc.readFrame()
		if err != nil {
			return 0, err
		}
		sc.readBuf.Write(pt)
	}
	return sc.readBuf.Read(p)
}

// Close closes the underlying connection.
func (sc *SecretConn) Close() error { return sc.conn.Close() }

// exchangeRaw writes payload and concurrently reads one frame from conn,
// returning the received payload. Doing the read in a goroutine lets both peers
// send first without deadlocking on an unbuffered duplex stream.
func exchangeRaw(conn io.ReadWriteCloser, payload []byte) ([]byte, error) {
	type result struct {
		b   []byte
		err error
	}
	rc := make(chan result, 1)
	go func() {
		b, _, err := kem.ReadFrame(conn)
		rc <- result{b, err}
	}()
	if _, err := kem.WriteFrame(conn, payload); err != nil {
		return nil, fmt.Errorf("write: %w", err)
	}
	res := <-rc
	if res.err != nil {
		return nil, fmt.Errorf("read: %w", res.err)
	}
	return res.b, nil
}

// ---- shared field helpers (length-prefixed, matching kem package) ----------

func writeField(w io.Writer, b []byte) {
	var l [4]byte
	binary.BigEndian.PutUint32(l[:], uint32(len(b)))
	_, _ = w.Write(l[:])
	_, _ = w.Write(b)
}

func readField(r *bytes.Reader) ([]byte, error) {
	var l [4]byte
	if _, err := io.ReadFull(r, l[:]); err != nil {
		return nil, err
	}
	n := binary.BigEndian.Uint32(l[:])
	if int(n) > r.Len() {
		return nil, fmt.Errorf("field length %d exceeds %d remaining", n, r.Len())
	}
	b := make([]byte, n)
	if _, err := io.ReadFull(r, b); err != nil {
		return nil, err
	}
	return b, nil
}
