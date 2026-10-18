module example.com/pcoc-quickstart

go 1.22

require github.com/groundinglab/pcoc/sdk/go/v2 v2.0.0-rc.1

// In your own module, drop this line once the tag sdk/go/v2.0.0-rc.1 is published.
replace github.com/groundinglab/pcoc/sdk/go/v2 => ../../go
