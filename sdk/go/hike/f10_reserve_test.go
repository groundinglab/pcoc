package hike

import (
	"encoding/binary"
	"os"
	"os/exec"
	"runtime"
	"testing"
)

// amp is the 64-deep count-equals-bytes-left input. maj 0x9a is an array,
// 0xba is a map. The declared count is the bytes still in the buffer.
func amp(size, levels int, maj byte) []byte {
	out := make([]byte, 0, size)
	for range levels {
		rem := size - len(out) - 5
		cnt := rem
		if maj == 0xba {
			cnt = rem / 2
		}
		out = append(out, maj)
		var be [4]byte
		binary.BigEndian.PutUint32(be[:], uint32(cnt))
		out = append(out, be[:]...)
		if maj == 0xba {
			out = append(out, 0x60)
		}
	}
	for len(out) < size {
		out = append(out, 0)
	}
	return out
}

func TestF10CountEqualToBytesLeftDoesNotPresize(t *testing.T) {
	if os.Getenv("F10_CHILD") != "1" {
		cmd := exec.Command("prlimit", "--as=4294967296", os.Args[0], "-test.run", "^TestF10CountEqualToBytesLeftDoesNotPresize$", "-test.v", "-test.count=1")
		cmd.Env = append(os.Environ(), "F10_CHILD=1")
		out, err := cmd.CombinedOutput()
		if err != nil {
			t.Fatalf("address space %v\n%s", err, out)
		}
		return
	}
	repros := []struct {
		name string
		size int
		maj  byte
	}{
		{"arr-1MiB", 1 << 20, 0x9a},
		{"arr-2MiB", 2 << 20, 0x9a},
		{"map-2MiB", 2 << 20, 0xba},
	}
	for _, repro := range repros {
		in := amp(repro.size, 64, repro.maj)
		var before, after runtime.MemStats
		runtime.GC()
		runtime.ReadMemStats(&before)
		issue := CBORIssue(in)
		runtime.ReadMemStats(&after)
		if issue == "" {
			t.Fatalf("%s decoded", repro.name)
		}
		grew := after.TotalAlloc - before.TotalAlloc
		// Surviving the 4 GiB address-space limit is the check. A reservation
		// taken from each nested count asks for tens of gigabytes and dies
		// here. The bytes actually retained stay under 2 GiB.
		if grew > 2<<30 {
			t.Fatalf("%s alloc %d issue %s", repro.name, grew, issue)
		}
		t.Logf("%s issue %s alloc %d", repro.name, issue, grew)
	}
	// A short 64-deep vector, the same shape, for the cross-check row.
	short := amp(4096, 64, 0x9a)
	if issue := CBORIssue(short); issue == "" {
		t.Fatal("short decoded")
	}
}
