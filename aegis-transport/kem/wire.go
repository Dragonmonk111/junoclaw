package kem

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"io"
)

// Wire encoding for the ADR-006 handshake messages.
//
// Each message body is a sequence of 4-byte big-endian length-prefixed fields,
// in the same order and with the same domain discipline as transcriptHash, so
// the bytes that cross the wire are exactly the bytes that get authenticated.
// Framing on a stream (the 4-byte total-length prefix per message) is the
// caller's concern — see WriteFrame/ReadFrame.

// MarshalBinary serializes Msg1 (version || x25519 pub || ML-KEM ek).
func (m *Msg1) MarshalBinary() ([]byte, error) {
	var buf bytes.Buffer
	writeField(&buf, []byte(m.Version))
	writeField(&buf, m.X25519Pub)
	writeField(&buf, m.MLKEMEk)
	return buf.Bytes(), nil
}

// MarshalBinary serializes Msg2 (version || x25519 pub || ML-KEM ct).
func (m *Msg2) MarshalBinary() ([]byte, error) {
	var buf bytes.Buffer
	writeField(&buf, []byte(m.Version))
	writeField(&buf, m.X25519Pub)
	writeField(&buf, m.MLKEMCt)
	return buf.Bytes(), nil
}

// UnmarshalMsg1 parses a Msg1 body produced by Msg1.MarshalBinary.
func UnmarshalMsg1(b []byte) (*Msg1, error) {
	r := bytes.NewReader(b)
	ver, err := readField(r)
	if err != nil {
		return nil, fmt.Errorf("msg1 version: %w", err)
	}
	xpub, err := readField(r)
	if err != nil {
		return nil, fmt.Errorf("msg1 x25519: %w", err)
	}
	ek, err := readField(r)
	if err != nil {
		return nil, fmt.Errorf("msg1 ek: %w", err)
	}
	if r.Len() != 0 {
		return nil, fmt.Errorf("msg1 has %d trailing bytes", r.Len())
	}
	return &Msg1{Version: string(ver), X25519Pub: xpub, MLKEMEk: ek}, nil
}

// UnmarshalMsg2 parses a Msg2 body produced by Msg2.MarshalBinary.
func UnmarshalMsg2(b []byte) (*Msg2, error) {
	r := bytes.NewReader(b)
	ver, err := readField(r)
	if err != nil {
		return nil, fmt.Errorf("msg2 version: %w", err)
	}
	xpub, err := readField(r)
	if err != nil {
		return nil, fmt.Errorf("msg2 x25519: %w", err)
	}
	ct, err := readField(r)
	if err != nil {
		return nil, fmt.Errorf("msg2 ct: %w", err)
	}
	if r.Len() != 0 {
		return nil, fmt.Errorf("msg2 has %d trailing bytes", r.Len())
	}
	return &Msg2{Version: string(ver), X25519Pub: xpub, MLKEMCt: ct}, nil
}

// readField reads one 4-byte big-endian length-prefixed field.
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

// WriteFrame writes a 4-byte big-endian length prefix followed by payload, and
// returns the total number of bytes written (prefix + payload).
func WriteFrame(w io.Writer, payload []byte) (int, error) {
	var l [4]byte
	binary.BigEndian.PutUint32(l[:], uint32(len(payload)))
	if _, err := w.Write(l[:]); err != nil {
		return 0, err
	}
	if _, err := w.Write(payload); err != nil {
		return 4, err
	}
	return 4 + len(payload), nil
}

// ReadFrame reads a 4-byte length-prefixed frame and returns the payload plus
// the total number of bytes read (prefix + payload).
func ReadFrame(r io.Reader) (payload []byte, total int, err error) {
	var l [4]byte
	if _, err := io.ReadFull(r, l[:]); err != nil {
		return nil, 0, err
	}
	n := binary.BigEndian.Uint32(l[:])
	payload = make([]byte, n)
	if _, err := io.ReadFull(r, payload); err != nil {
		return nil, 4, err
	}
	return payload, 4 + int(n), nil
}
