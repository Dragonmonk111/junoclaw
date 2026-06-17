// Project Aegis — Phase C transport harness.
//
// Isolated single-module harness with NO external dependencies: it uses only
// the Go 1.24 standard library (crypto/mlkem, crypto/ecdh, crypto/hkdf,
// crypto/sha256), matching the runtime selection in
// docs/ADR-006-PQC-HYBRID-TRANSPORT.md.
module github.com/junoclaw/aegis-transport

go 1.24
