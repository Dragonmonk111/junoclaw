module github.com/junoclaw/hybrid-consensus-keygen

go 1.24.0

require github.com/cometbft/cometbft v0.38.17

require (
	github.com/cloudflare/circl v1.6.1 // indirect
	github.com/oasisprotocol/curve25519-voi v0.0.0-20220708102147-0a8a51822cae // indirect
	github.com/petermattis/goid v0.0.0-20250813065127-a731cc31b4fe // indirect
	github.com/sasha-s/go-deadlock v0.3.9 // indirect
	golang.org/x/crypto v0.33.0 // indirect
	golang.org/x/sys v0.30.0 // indirect
)

replace github.com/cometbft/cometbft => ../../../aegis-forks/cometbft
