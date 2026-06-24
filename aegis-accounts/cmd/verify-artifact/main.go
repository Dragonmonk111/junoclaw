// verify-artifact: Project Aegis cross-CPU/OS determinism test.
//
// Proves that hybrid.PubKey.Verify (secp256k1 + ML-DSA-44) returns
// bit-identical results on every architecture. Uses fixed seeds so the
// keypair and message are identical on every platform; the verification
// result and the hash of the fixed inputs MUST match — only goos/goarch
// may differ.
//
// Run:
//   go run ./cmd/verify-artifact > artifacts/verify-amd64.json   # x86_64
//   (cross-built + QEMU or real ARM)   > artifacts/verify-arm64.json
//
// Compare:
//   jq -S 'del(.goos,.goarch,.go_version)' artifacts/verify-amd64.json > /tmp/a.json
//   jq -S 'del(.goos,.goarch,.go_version)' artifacts/verify-arm64.json > /tmp/b.json
//   diff /tmp/a.json /tmp/b.json && echo "CROSS-ARCH DETERMINISM CONFIRMED"
package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"runtime"

	aegisaccounts "github.com/junoclaw/aegis-accounts"
)

// Fixed seeds — hardcoded so every platform uses identical inputs.
// secp256k1 seed: 32 bytes of 0x01
// ML-DSA-44 seed: 32 bytes of 0x02  (mldsa44.SeedSize = 32)
var (
	secpSeedFixed  = func() []byte { b := make([]byte, 32); for i := range b { b[i] = 0x01 }; return b }()
	mldsaSeedFixed = func() *[32]byte { var s [32]byte; for i := range s { s[i] = 0x02 }; return &s }()
	fixedMsg       = []byte("Project Aegis cross-arch determinism test 2026-06-23")
)

func main() {
	// Deterministic keygen from fixed seeds.
	priv, err := aegisaccounts.NewHybridFromSeeds(secpSeedFixed, mldsaSeedFixed)
	if err != nil {
		fmt.Fprintf(os.Stderr, "keygen error: %v\n", err)
		os.Exit(1)
	}
	pub := priv.PubKey()

	// Sign the fixed message. ML-DSA internally hedges with rand.Reader so
	// sig *bytes* may differ across runs — but Verify output is always
	// deterministic (integer-only). We test Verify, not sig bytes.
	sig, err := priv.Sign(fixedMsg)
	if err != nil {
		fmt.Fprintf(os.Stderr, "sign error: %v\n", err)
		os.Exit(1)
	}

	// Primary assertions — these MUST be identical on every platform.
	verifyOK := pub.Verify(fixedMsg, sig)

	tampered := make([]byte, len(fixedMsg))
	copy(tampered, fixedMsg)
	tampered[0] ^= 0x01
	tamperedRejected := !pub.Verify(tampered, sig)

	// input_sha256: SHA-256 of the fixed inputs (pubkey bytes + msg).
	// Does NOT include sig bytes (which are non-deterministic due to hedged
	// ML-DSA randomness). This hash proves keygen is deterministic across arches.
	pubBytes, err := pub.Bytes()
	if err != nil {
		fmt.Fprintf(os.Stderr, "pub.Bytes error: %v\n", err)
		os.Exit(1)
	}
	h := sha256.New()
	h.Write(pubBytes)
	h.Write(fixedMsg)
	inputHash := hex.EncodeToString(h.Sum(nil))

	out := map[string]any{
		"goos":              runtime.GOOS,
		"goarch":            runtime.GOARCH,
		"go_version":        runtime.Version(),
		"verify_ok":         verifyOK,
		"tampered_rejected": tamperedRejected,
		"input_sha256":      inputHash,
	}

	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	if err := enc.Encode(out); err != nil {
		fmt.Fprintf(os.Stderr, "encode error: %v\n", err)
		os.Exit(1)
	}

	// Non-zero exit if assertions fail — causes CI / shell to catch failures.
	if !verifyOK {
		fmt.Fprintln(os.Stderr, "FAIL: verify_ok is false — verify returned wrong result")
		os.Exit(2)
	}
	if !tamperedRejected {
		fmt.Fprintln(os.Stderr, "FAIL: tampered_rejected is false — tampered message was accepted")
		os.Exit(2)
	}
}
