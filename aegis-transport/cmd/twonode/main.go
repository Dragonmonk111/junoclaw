// Command twonode is the Project Aegis Phase C / C3 two-node handshake
// prototype. It runs the ADR-006 hybrid X25519 + ML-KEM-768 secret-connection
// handshake between a real initiator and responder over a loopback TCP socket,
// and measures the two things ADR-006 explicitly defers to C3:
//
//   - wall-clock handshake latency (1-RTT key agreement), and
//   - bytes on the wire each direction,
//
// each compared against a classical X25519-only baseline so the *delta* the ADR
// predicts ("the latency cost is bytes, not crypto") can be read off directly.
//
// Correctness is checked over the actual socket: after deriving the session
// key, the initiator sends an HMAC key-confirmation tag and the responder
// verifies it, so a measured run also proves both peers agreed on the key.
//
// Run: go run ./cmd/twonode [iterations]   (default 200)
package main

import (
	"crypto/ecdh"
	"crypto/hkdf"
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"fmt"
	"net"
	"os"
	"sort"
	"time"

	"github.com/junoclaw/aegis-transport/kem"
)

const (
	confirmLabel   = "junoclaw/aegis/key-confirm/v1"
	classicalKDF   = "junoclaw/aegis/classical-baseline/v1"
	classicalProto = "secret-connection/v1-classical"
)

// confirmTag is the key-confirmation MAC the initiator sends to prove it
// derived the same session key the responder did.
func confirmTag(key []byte) []byte {
	m := hmac.New(sha256.New, key)
	m.Write([]byte(confirmLabel))
	return m.Sum(nil)
}

// ---- generic server loop ---------------------------------------------------

func serve(ln net.Listener, n int, handle func(net.Conn) error, errc chan<- error) {
	for i := 0; i < n; i++ {
		c, err := ln.Accept()
		if err != nil {
			errc <- err
			return
		}
		if err := handle(c); err != nil {
			c.Close()
			errc <- err
			return
		}
		c.Close()
	}
	errc <- nil
}

// readConfirmAndAck reads the initiator's key-confirmation tag, verifies it
// against the responder's own key, and replies with a 1-byte ack.
func readConfirmAndAck(conn net.Conn, key []byte) error {
	tag, _, err := kem.ReadFrame(conn)
	if err != nil {
		return fmt.Errorf("read confirm: %w", err)
	}
	ok := hmac.Equal(tag, confirmTag(key))
	ack := []byte{0}
	if ok {
		ack[0] = 1
	}
	if _, err := kem.WriteFrame(conn, ack); err != nil {
		return fmt.Errorf("write ack: %w", err)
	}
	if !ok {
		return fmt.Errorf("responder: key confirmation failed (no agreement)")
	}
	return nil
}

// sendConfirmCheckAck sends the initiator's key-confirmation tag and verifies
// the responder accepted it.
func sendConfirmCheckAck(conn net.Conn, key []byte) error {
	if _, err := kem.WriteFrame(conn, confirmTag(key)); err != nil {
		return fmt.Errorf("write confirm: %w", err)
	}
	ack, _, err := kem.ReadFrame(conn)
	if err != nil {
		return fmt.Errorf("read ack: %w", err)
	}
	if len(ack) != 1 || ack[0] != 1 {
		return fmt.Errorf("initiator: responder rejected key confirmation")
	}
	return nil
}

// ---- hybrid (ADR-006) ------------------------------------------------------

func handleHybrid(conn net.Conn) error {
	b1, _, err := kem.ReadFrame(conn)
	if err != nil {
		return fmt.Errorf("read msg1: %w", err)
	}
	m1, err := kem.UnmarshalMsg1(b1)
	if err != nil {
		return err
	}
	m2, key, err := kem.ResponderRespond(m1)
	if err != nil {
		return err
	}
	b2, err := m2.MarshalBinary()
	if err != nil {
		return err
	}
	if _, err := kem.WriteFrame(conn, b2); err != nil {
		return fmt.Errorf("write msg2: %w", err)
	}
	return readConfirmAndAck(conn, key)
}

func clientHybrid(addr string) (lat time.Duration, sent, recv int, err error) {
	conn, err := net.Dial("tcp", addr)
	if err != nil {
		return
	}
	defer conn.Close()

	st, m1, err := kem.InitiatorStart()
	if err != nil {
		return
	}
	b1, err := m1.MarshalBinary()
	if err != nil {
		return
	}

	t0 := time.Now()
	if sent, err = kem.WriteFrame(conn, b1); err != nil {
		return
	}
	b2, recvN, err := kem.ReadFrame(conn)
	if err != nil {
		return
	}
	recv = recvN
	m2, err := kem.UnmarshalMsg2(b2)
	if err != nil {
		return
	}
	key, err := st.InitiatorFinish(m1, m2)
	if err != nil {
		return
	}
	lat = time.Since(t0) // 1-RTT hybrid key agreement

	err = sendConfirmCheckAck(conn, key)
	return
}

// ---- classical X25519-only baseline ---------------------------------------

func classicalKey(ss, cliPub, srvPub []byte) ([]byte, error) {
	h := sha256.New()
	h.Write([]byte(classicalProto))
	h.Write(cliPub)
	h.Write(srvPub)
	return hkdf.Key(sha256.New, ss, h.Sum(nil), classicalKDF, 32)
}

func handleClassical(conn net.Conn) error {
	cliPubB, _, err := kem.ReadFrame(conn)
	if err != nil {
		return fmt.Errorf("read cli pub: %w", err)
	}
	priv, err := ecdh.X25519().GenerateKey(rand.Reader)
	if err != nil {
		return err
	}
	cliPub, err := ecdh.X25519().NewPublicKey(cliPubB)
	if err != nil {
		return err
	}
	ss, err := priv.ECDH(cliPub)
	if err != nil {
		return err
	}
	srvPubB := priv.PublicKey().Bytes()
	if _, err := kem.WriteFrame(conn, srvPubB); err != nil {
		return fmt.Errorf("write srv pub: %w", err)
	}
	key, err := classicalKey(ss, cliPubB, srvPubB)
	if err != nil {
		return err
	}
	return readConfirmAndAck(conn, key)
}

func clientClassical(addr string) (lat time.Duration, sent, recv int, err error) {
	conn, err := net.Dial("tcp", addr)
	if err != nil {
		return
	}
	defer conn.Close()

	priv, err := ecdh.X25519().GenerateKey(rand.Reader)
	if err != nil {
		return
	}
	cliPubB := priv.PublicKey().Bytes()

	t0 := time.Now()
	if sent, err = kem.WriteFrame(conn, cliPubB); err != nil {
		return
	}
	srvPubB, recvN, err := kem.ReadFrame(conn)
	if err != nil {
		return
	}
	recv = recvN
	srvPub, err := ecdh.X25519().NewPublicKey(srvPubB)
	if err != nil {
		return
	}
	ss, err := priv.ECDH(srvPub)
	if err != nil {
		return
	}
	key, err := classicalKey(ss, cliPubB, srvPubB)
	if err != nil {
		return
	}
	lat = time.Since(t0) // 1-RTT classical key agreement

	err = sendConfirmCheckAck(conn, key)
	return
}

// ---- measurement -----------------------------------------------------------

type stat struct {
	lats       []time.Duration
	sent, recv int
}

func (s stat) mean() time.Duration {
	if len(s.lats) == 0 {
		return 0
	}
	var sum time.Duration
	for _, d := range s.lats {
		sum += d
	}
	return sum / time.Duration(len(s.lats))
}

func (s stat) pct(p float64) time.Duration {
	if len(s.lats) == 0 {
		return 0
	}
	c := append([]time.Duration(nil), s.lats...)
	sort.Slice(c, func(i, j int) bool { return c[i] < c[j] })
	idx := int(p * float64(len(c)-1))
	return c[idx]
}

func runVariant(iters int, handle func(net.Conn) error, client func(string) (time.Duration, int, int, error)) (stat, error) {
	var s stat
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return s, err
	}
	defer ln.Close()

	errc := make(chan error, 1)
	go serve(ln, iters, handle, errc)
	addr := ln.Addr().String()

	for i := 0; i < iters; i++ {
		lat, sent, recv, err := client(addr)
		if err != nil {
			return s, fmt.Errorf("iteration %d: %w", i, err)
		}
		s.lats = append(s.lats, lat)
		s.sent, s.recv = sent, recv
	}
	if err := <-errc; err != nil {
		return s, fmt.Errorf("responder: %w", err)
	}
	return s, nil
}

func main() {
	iters := 200
	if len(os.Args) > 1 {
		if n, err := fmt.Sscanf(os.Args[1], "%d", &iters); n != 1 || err != nil || iters < 1 {
			fmt.Fprintln(os.Stderr, "usage: twonode [iterations>=1]")
			os.Exit(2)
		}
	}

	fmt.Println("Project Aegis — Phase C/C3: two-node hybrid handshake over loopback TCP")
	fmt.Println("(real initiator/responder sockets; key confirmed over the wire; see docs/ADR-006)")
	fmt.Printf("iterations: %d\n", iters)

	hyb, err := runVariant(iters, handleHybrid, clientHybrid)
	if err != nil {
		fmt.Fprintln(os.Stderr, "hybrid run failed:", err)
		os.Exit(1)
	}
	cls, err := runVariant(iters, handleClassical, clientClassical)
	if err != nil {
		fmt.Fprintln(os.Stderr, "classical run failed:", err)
		os.Exit(1)
	}

	fmt.Println("\n== Bandwidth (key-agreement frames, incl. 4-byte length prefixes) ==")
	fmt.Printf("  %-26s %6s %6s %8s\n", "variant", "→ B", "← B", "total B")
	fmt.Printf("  %-26s %6d %6d %8d\n", "classical X25519", cls.sent, cls.recv, cls.sent+cls.recv)
	fmt.Printf("  %-26s %6d %6d %8d\n", "hybrid X25519+ML-KEM-768", hyb.sent, hyb.recv, hyb.sent+hyb.recv)
	fmt.Printf("  %-26s %6d %6d %8d\n", "delta (PQ overhead)", hyb.sent-cls.sent, hyb.recv-cls.recv,
		(hyb.sent+hyb.recv)-(cls.sent+cls.recv))

	fmt.Println("\n== Latency (1-RTT key agreement over loopback; per handshake) ==")
	fmt.Printf("  %-26s %12s %12s %12s\n", "variant", "mean", "p50", "p90")
	fmt.Printf("  %-26s %12v %12v %12v\n", "classical X25519", cls.mean(), cls.pct(0.50), cls.pct(0.90))
	fmt.Printf("  %-26s %12v %12v %12v\n", "hybrid X25519+ML-KEM-768", hyb.mean(), hyb.pct(0.50), hyb.pct(0.90))
	fmt.Printf("  %-26s %12v\n", "delta (mean)", hyb.mean()-cls.mean())

	fmt.Println("\nNote: loopback removes real network RTT, so the latency delta here is")
	fmt.Println("crypto + framing only. On a real link the ~2.3 KB extra (one MTU or two)")
	fmt.Println("dominates, exactly as ADR-006 predicts: the cost is bytes, not crypto CPU.")
}
