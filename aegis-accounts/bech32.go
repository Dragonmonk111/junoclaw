package aegisaccounts

// Minimal BIP-173 bech32 implementation (no external dependency).
//
// Only what the harness needs: encode 8-bit address bytes to a bech32 string
// and decode back for round-trip tests. Validated in bech32_test.go against the
// BIP-173 reference vectors and by exhaustive round-trip on random inputs.

import (
	"fmt"
	"strings"
)

const charset = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"

var bech32Gen = []uint32{0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3}

func bech32Polymod(values []byte) uint32 {
	chk := uint32(1)
	for _, v := range values {
		top := chk >> 25
		chk = (chk&0x1ffffff)<<5 ^ uint32(v)
		for i := 0; i < 5; i++ {
			if (top>>uint(i))&1 == 1 {
				chk ^= bech32Gen[i]
			}
		}
	}
	return chk
}

func bech32HRPExpand(hrp string) []byte {
	out := make([]byte, 0, len(hrp)*2+1)
	for i := 0; i < len(hrp); i++ {
		out = append(out, hrp[i]>>5)
	}
	out = append(out, 0)
	for i := 0; i < len(hrp); i++ {
		out = append(out, hrp[i]&31)
	}
	return out
}

func bech32Checksum(hrp string, data []byte) []byte {
	values := append(bech32HRPExpand(hrp), data...)
	values = append(values, 0, 0, 0, 0, 0, 0)
	pm := bech32Polymod(values) ^ 1
	out := make([]byte, 6)
	for i := 0; i < 6; i++ {
		out[i] = byte((pm >> uint(5*(5-i))) & 31)
	}
	return out
}

// convertBits regroups a byte slice from groups of fromBits to groups of
// toBits, optionally padding the final group. This is the standard bech32
// 8<->5 bit packing routine.
func convertBits(data []byte, fromBits, toBits uint, pad bool) ([]byte, error) {
	var acc uint32
	var bits uint
	out := make([]byte, 0, len(data)*int(fromBits)/int(toBits)+1)
	maxv := uint32(1<<toBits) - 1
	maxAcc := uint32(1<<(fromBits+toBits-1)) - 1
	for _, value := range data {
		acc = ((acc << fromBits) | uint32(value)) & maxAcc
		bits += fromBits
		for bits >= toBits {
			bits -= toBits
			out = append(out, byte((acc>>bits)&maxv))
		}
	}
	if pad {
		if bits > 0 {
			out = append(out, byte((acc<<(toBits-bits))&maxv))
		}
	} else if bits >= fromBits || ((acc<<(toBits-bits))&maxv) != 0 {
		return nil, fmt.Errorf("bech32: invalid padding")
	}
	return out, nil
}

// EncodeBech32 encodes 8-bit data (e.g. a 20-byte address) under the given
// human-readable prefix.
func EncodeBech32(hrp string, data8 []byte) (string, error) {
	if hrp == "" {
		return "", fmt.Errorf("bech32: empty hrp")
	}
	data5, err := convertBits(data8, 8, 5, true)
	if err != nil {
		return "", err
	}
	combined := append(data5, bech32Checksum(hrp, data5)...)
	var sb strings.Builder
	sb.WriteString(hrp)
	sb.WriteByte('1')
	for _, b := range combined {
		if int(b) >= len(charset) {
			return "", fmt.Errorf("bech32: invalid data symbol %d", b)
		}
		sb.WriteByte(charset[b])
	}
	return sb.String(), nil
}

// DecodeBech32 reverses EncodeBech32, returning the prefix and the original
// 8-bit data. Used by tests for round-trip verification.
func DecodeBech32(bech string) (string, []byte, error) {
	lower, upper := strings.ToLower(bech), strings.ToUpper(bech)
	if bech != lower && bech != upper {
		return "", nil, fmt.Errorf("bech32: mixed case")
	}
	bech = lower
	pos := strings.LastIndexByte(bech, '1')
	if pos < 1 || pos+7 > len(bech) {
		return "", nil, fmt.Errorf("bech32: invalid separator position")
	}
	hrp := bech[:pos]
	data := make([]byte, 0, len(bech)-pos-1)
	for _, c := range bech[pos+1:] {
		idx := strings.IndexRune(charset, c)
		if idx < 0 {
			return "", nil, fmt.Errorf("bech32: invalid character %q", c)
		}
		data = append(data, byte(idx))
	}
	if bech32Polymod(append(bech32HRPExpand(hrp), data...)) != 1 {
		return "", nil, fmt.Errorf("bech32: invalid checksum")
	}
	data8, err := convertBits(data[:len(data)-6], 5, 8, false)
	if err != nil {
		return "", nil, err
	}
	return hrp, data8, nil
}
