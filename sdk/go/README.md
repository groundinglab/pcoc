# Go SDK

Module `github.com/groundinglab/pcoc/sdk/go/v2`, package `pcoc`. The command is `pcoc` (`cmd/pcoc`).

From this directory, the full test command is:

```sh
go test ./...
```

That is what CI runs. It needs the Go version in `go.mod`. `check/negatives.sh` checks that the thirteen permanent negatives stay out of the release library and the `pcoc` command.

## Quickstart

From this directory:

```sh
go run ./cmd/pcoc quickstart
```

Six lines. The first names `go`. The second names the SHA-256 backend on this machine. The other four lines are the same text the Rust, C, and Java quickstarts print. The production-leg line is the pinned `legs_le_hex`. The command does not time a leg.
