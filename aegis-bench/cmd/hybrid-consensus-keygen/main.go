// hybrid-consensus-keygen upgrades an existing CometBFT Ed25519
// priv_validator_key.json to a Project Aegis Phase F hybrid
// (Ed25519 + ML-DSA-44) consensus key.
//
// MIGRATION INVARIANT (ADR-008 §F1-b): Address() delegates to the Ed25519
// half, so the validator address is UNCHANGED.  The hybrid pubkey is larger
// (1344 B vs 32 B) and the hybrid signature is larger (2491 B vs 64 B),
// which is what drives the ~12 KB commit_bytes measurement.
//
// Usage:
//
//	hybrid-consensus-keygen <priv_validator_key.json> [...]
//
// Each file is upgraded in-place (written atomically via rename).
package main

import (
	"crypto/rand"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/cometbft/cometbft/crypto/ed25519"
	"github.com/cometbft/cometbft/crypto/hybrid"
	"github.com/cometbft/cometbft/crypto/mldsa44"
)

// keyJSON mirrors the priv_validator_key.json format written by CometBFT.
type keyJSON struct {
	Address string       `json:"address"`
	PubKey  keyTypeValue `json:"pub_key"`
	PrivKey keyTypeValue `json:"priv_key"`
}

type keyTypeValue struct {
	Type  string `json:"type"`
	Value string `json:"value"` // base64-encoded bytes
}

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "usage: hybrid-consensus-keygen <priv_validator_key.json> [...]")
		os.Exit(1)
	}
	for _, path := range os.Args[1:] {
		if err := upgradeKey(path); err != nil {
			fmt.Fprintf(os.Stderr, "ERROR %s: %v\n", path, err)
			os.Exit(1)
		}
	}
}

func upgradeKey(path string) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("read: %w", err)
	}
	var orig keyJSON
	if err := json.Unmarshal(data, &orig); err != nil {
		return fmt.Errorf("parse: %w", err)
	}
	if orig.PrivKey.Type != "tendermint/PrivKeyEd25519" {
		return fmt.Errorf("unsupported key type %q (expected tendermint/PrivKeyEd25519)", orig.PrivKey.Type)
	}

	edBytes, err := base64.StdEncoding.DecodeString(orig.PrivKey.Value)
	if err != nil {
		return fmt.Errorf("decode priv key: %w", err)
	}
	if len(edBytes) != ed25519.PrivateKeySize {
		return fmt.Errorf("ed25519 priv key: got %d bytes, want %d", len(edBytes), ed25519.PrivateKeySize)
	}
	edPriv := ed25519.PrivKey(edBytes)

	var mlSeedArr [mldsa44.SeedSize]byte
	if _, err := rand.Read(mlSeedArr[:]); err != nil {
		return fmt.Errorf("rand: %w", err)
	}
	mlPriv, err := mldsa44.GenPrivKeyFromSeed(mlSeedArr[:])
	if err != nil {
		return fmt.Errorf("mldsa44 keygen: %w", err)
	}

	hybridPriv, err := hybrid.NewPrivKeyFromHalves(edPriv, mlPriv)
	if err != nil {
		return fmt.Errorf("hybrid keygen: %w", err)
	}
	hybridPub := hybridPriv.PubKey()

	addr := hybridPub.Address()
	newKey := keyJSON{
		Address: strings.ToUpper(hex.EncodeToString(addr)),
		PubKey: keyTypeValue{
			Type:  hybrid.PubKeyName,
			Value: base64.StdEncoding.EncodeToString(hybridPub.Bytes()),
		},
		PrivKey: keyTypeValue{
			Type:  hybrid.PrivKeyName,
			Value: base64.StdEncoding.EncodeToString(hybridPriv.Bytes()),
		},
	}

	out, err := json.MarshalIndent(newKey, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal: %w", err)
	}
	tmp := path + ".tmp"
	if err := os.WriteFile(tmp, append(out, '\n'), 0600); err != nil {
		return fmt.Errorf("write: %w", err)
	}
	if err := os.Rename(tmp, path); err != nil {
		return fmt.Errorf("rename: %w", err)
	}

	fmt.Printf("✓ %-50s address=%s (unchanged)\n", filepath.Base(path), newKey.Address)
	fmt.Printf("  pub  %d B  type=%s\n", len(hybridPub.Bytes()), hybrid.PubKeyName)
	fmt.Printf("  priv %d B  type=%s\n", len(hybridPriv.Bytes()), hybrid.PrivKeyName)
	return nil
}
