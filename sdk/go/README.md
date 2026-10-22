# Go SDK

Module `github.com/groundinglab/pcoc/sdk/go/v2`, package `pcoc`. The command is `pcoc` (`cmd/pcoc`).

From this directory, the full test command is:

```sh
go test ./...
```

That is what CI runs. It needs the Go version in `go.mod`. `check/negatives.sh` checks that the thirteen permanent negatives stay out of the release library and the `pcoc` command.
