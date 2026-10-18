package hike

import "encoding/json"

// ProduceReport is satoshi-hike-cli-report-v0.2.
// result is a section 4 token. present means the object was written.
// not read means it was not. The text does not use a verdict word.
func ProduceReport(output string, written bool) string {
	if !written {
		return "{\"schema\":\"satoshi-hike-cli-report-v0.2\",\"command\":\"produce\",\"result\":\"not read\",\"rung\":null}\n"
	}
	quoted, err := json.Marshal(output)
	if err != nil {
		quoted = []byte(`""`)
	}
	return "{\"schema\":\"satoshi-hike-cli-report-v0.2\",\"command\":\"produce\",\"result\":\"present\",\"rung\":null,\"output\":" + string(quoted) + "}\n"
}
