package pcoc

// Selftest and build info (SPEC §11.1, Rule I5).
//
// The SHA-256 implementation is crypto/sha256. On amd64 that package uses
// SHA-NI when the CPU has it. The backend name reported here is taken from
// the CPU flags, which is how this SDK selects the hardware path: the
// standard library reads the same flags.

import (
	"fmt"
	"runtime"
	"strings"

	"golang.org/x/sys/cpu"
)

// SHABackend is the SHA-256 backend this process uses (SPEC §11.1).
// sha-ni on amd64 when the CPU has SHA-NI, armv8-sha2 on arm64 when the
// CPU has SHA2, and software otherwise.
func SHABackend() string {
	switch runtime.GOARCH {
	case "amd64":
		if hasSHANI() {
			return "sha-ni"
		}
	case "arm64":
		if cpu.ARM64.HasSHA2 {
			return "armv8-sha2"
		}
	}
	return "software"
}

// HasSHANI reports whether this CPU provides x86 SHA-NI.
func HasSHANI() bool { return hasSHANI() }

// CPUFlags is the set of flags this process can see, for the selftest line.
func CPUFlags() string {
	var parts []string
	if runtime.GOARCH == "amd64" {
		if cpu.X86.HasAVX {
			parts = append(parts, "avx")
		}
		if cpu.X86.HasAVX2 {
			parts = append(parts, "avx2")
		}
		if cpu.X86.HasAVX512F {
			parts = append(parts, "avx512f")
		}
		if hasSHANI() {
			parts = append(parts, "sha_ni")
		}
	}
	if runtime.GOARCH == "arm64" {
		if cpu.ARM64.HasASIMD {
			parts = append(parts, "asimd")
		}
		if cpu.ARM64.HasSHA2 {
			parts = append(parts, "sha2")
		}
	}
	if len(parts) == 0 {
		return "none"
	}
	return strings.Join(parts, ",")
}

// SelftestText is the pcoc selftest report. The fold kernel is the scalar
// one. The S1 known-answer check is the honest first leg.
func SelftestText(pin Pin) string {
	var b strings.Builder
	fmt.Fprintf(&b, "sdk: go\n")
	fmt.Fprintf(&b, "version: %s\n", Version)
	fmt.Fprintf(&b, "spec: %s\n", SpecTag)
	fmt.Fprintf(&b, "pack: %s\n", pin.VectorSHA256)
	fmt.Fprintf(&b, "goarch: %s\n", runtime.GOARCH)
	fmt.Fprintf(&b, "cpu_flags: %s\n", CPUFlags())
	fmt.Fprintf(&b, "sha256: %s\n", SHABackend())
	fmt.Fprintf(&b, "fold: scalar\n")
	return b.String()
}
