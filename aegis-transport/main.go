// Command aegis-transport is the Phase C / C2 demo for the hybrid
// X25519 + ML-KEM-768 secret-connection KEM combiner of ADR-006.
//
// It runs a full in-process handshake, prints the wire sizes against the
// ADR-006 bandwidth table, and reports ML-KEM-768 keygen/encap/decap and
// full-handshake timing. Run the conformance/property tests with `go test ./...`.
package main

import (
	"crypto/mlkem"
	"fmt"
	"os"
	"time"

	"github.com/junoclaw/aegis-transport/kem"
)

func main() {
	fmt.Println("Project Aegis — Phase C/C2: hybrid X25519 + ML-KEM-768 secret connection")
	fmt.Println("(Go stdlib crypto/mlkem + crypto/ecdh + crypto/hkdf; see docs/ADR-006-PQC-HYBRID-TRANSPORT.md)")

	if err := kem.SelfCheckSizes(); err != nil {
		fmt.Fprintln(os.Stderr, "size self-check failed:", err)
		os.Exit(1)
	}

	iKey, rKey, m1, m2, err := kem.HybridHandshake()
	if err != nil {
		fmt.Fprintln(os.Stderr, "handshake failed:", err)
		os.Exit(1)
	}
	agreed := kem.Equal(iKey, rKey)
	fmt.Printf("\nhandshake agreement: %v  (session key %d B, info=%q)\n", agreed, len(iKey), kem.KDFInfo)
	if !agreed {
		fmt.Fprintln(os.Stderr, "FATAL: peers did not derive the same session key")
		os.Exit(1)
	}

	fmt.Println("\n== Wire sizes (ADR-006 bandwidth table) ==")
	fmt.Printf("  %-22s %5d B\n", "Msg1 X25519 pub", len(m1.X25519Pub))
	fmt.Printf("  %-22s %5d B\n", "Msg1 ML-KEM-768 ek", len(m1.MLKEMEk))
	fmt.Printf("  %-22s %5d B\n", "Msg2 X25519 pub", len(m2.X25519Pub))
	fmt.Printf("  %-22s %5d B\n", "Msg2 ML-KEM-768 ct", len(m2.MLKEMCt))
	pqAdded := len(m1.MLKEMEk) + len(m2.MLKEMCt)
	classical := 2 * kem.X25519PubSize
	fmt.Printf("  %-22s %5d B  (vs %d B classical X25519; +%d B for KEM)\n",
		"key-agreement total", pqAdded+classical, classical, pqAdded)

	timing()
}

func timing() {
	const iters = 200

	t := time.Now()
	for i := 0; i < iters; i++ {
		if _, err := mlkem.GenerateKey768(); err != nil {
			panic(err)
		}
	}
	keygen := time.Since(t) / iters

	dk, err := mlkem.GenerateKey768()
	if err != nil {
		panic(err)
	}
	ek := dk.EncapsulationKey()

	var ct []byte
	t = time.Now()
	for i := 0; i < iters; i++ {
		_, ct = ek.Encapsulate()
	}
	encap := time.Since(t) / iters

	t = time.Now()
	for i := 0; i < iters; i++ {
		if _, err := dk.Decapsulate(ct); err != nil {
			panic(err)
		}
	}
	decap := time.Since(t) / iters

	t = time.Now()
	for i := 0; i < iters; i++ {
		if _, _, _, _, err := kem.HybridHandshake(); err != nil {
			panic(err)
		}
	}
	handshake := time.Since(t) / iters

	fmt.Printf("\n== Timing (mean of %d iters; wall-clock on this machine) ==\n", iters)
	fmt.Printf("  %-26s %v\n", "ML-KEM-768 keygen", keygen)
	fmt.Printf("  %-26s %v\n", "ML-KEM-768 encapsulate", encap)
	fmt.Printf("  %-26s %v\n", "ML-KEM-768 decapsulate", decap)
	fmt.Printf("  %-26s %v\n", "full hybrid handshake", handshake)
	fmt.Println("\n(handshake includes 2x X25519 keygen+ECDH and ML-KEM keygen+encap+decap;")
	fmt.Println(" KEM CPU is tens of microseconds — ADR-006's latency cost is bytes, not crypto.)")
}
