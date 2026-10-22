// Command timeprod reports the wall time of one PROD terrain fill and one
// PROD leg (SPEC §6 constants, pk_A, the pack seed beacon). AC-C7.
package main

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"runtime"
	"time"

	"pcoc.dev/sdk/go"
)

func main() {
	runtime.GOMAXPROCS(4)
	pk, _ := hex.DecodeString("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9")
	sum := sha256.Sum256([]byte("pcoc-pack-2.2/beacon/seed"))
	p := pcoc.Production()
	t0 := time.Now()
	terrain, err := pcoc.FillTerrain(p, pk, sum[:])
	tFill := time.Since(t0)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	dig := sha256.Sum256(terrain.Bytes())
	t1 := time.Now()
	lr := pcoc.LegResult(terrain, terrain.Seed())
	tLeg := time.Since(t1)
	fmt.Printf("terrain_wall_s\t%.3f\n", tFill.Seconds())
	fmt.Printf("leg_wall_s\t%.3f\n", tLeg.Seconds())
	fmt.Printf("terrain\t%x\n", dig[:])
	fmt.Printf("legs_le_hex\t%x\n", pcoc.Le64(lr))
	fmt.Printf("gomaxprocs\t%d\n", runtime.GOMAXPROCS(0))
}
