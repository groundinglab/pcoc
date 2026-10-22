package pcoc

// SHA-NI detection on amd64 (SPEC §11.1).
//
// crypto/sha256 uses SHA-NI when the CPU reports it. golang.org/x/sys/cpu
// no longer exports that flag, so this file reads the same CPUID bit the
// runtime reads: leaf 7, EBX bit 29. The other flags in the selftest still
// come from x/sys/cpu.

func cpuid(leaf uint32) (eax, ebx, ecx, edx uint32)

func hasSHANI() bool {
	_, ebx, _, _ := cpuid(7)
	return ebx&(1<<29) != 0
}
