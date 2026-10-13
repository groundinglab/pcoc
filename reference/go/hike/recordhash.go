package hike

import (
	"bytes"
	"crypto/sha256"
	"encoding/json"
	"sort"
	"strconv"
	"strings"
)

// VerifyRecordSHA256 is SHA-256 of the section 14 object under RFC 8785.
// The trailing newline is part of the CLI file. It is not an input to this hash.
func VerifyRecordSHA256(record string) ([32]byte, error) {
	body := record
	if strings.HasSuffix(body, "\n") {
		body = body[:len(body)-1]
	}
	dec := json.NewDecoder(strings.NewReader(body))
	dec.UseNumber()
	var value any
	if err := dec.Decode(&value); err != nil {
		return [32]byte{}, err
	}
	canon, err := jcsBytes(value)
	if err != nil {
		return [32]byte{}, err
	}
	return sha256.Sum256(canon), nil
}

func jcsBytes(value any) ([]byte, error) {
	var buf bytes.Buffer
	if err := writeJCS(&buf, value); err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

func writeJCS(buf *bytes.Buffer, value any) error {
	switch typed := value.(type) {
	case map[string]any:
		keys := make([]string, 0, len(typed))
		for key := range typed {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		buf.WriteByte('{')
		for i, key := range keys {
			if i > 0 {
				buf.WriteByte(',')
			}
			raw, err := json.Marshal(key)
			if err != nil {
				return err
			}
			buf.Write(raw)
			buf.WriteByte(':')
			if err := writeJCS(buf, typed[key]); err != nil {
				return err
			}
		}
		buf.WriteByte('}')
	case []any:
		buf.WriteByte('[')
		for i, item := range typed {
			if i > 0 {
				buf.WriteByte(',')
			}
			if err := writeJCS(buf, item); err != nil {
				return err
			}
		}
		buf.WriteByte(']')
	case json.Number:
		buf.WriteString(typed.String())
	case string:
		raw, err := json.Marshal(typed)
		if err != nil {
			return err
		}
		buf.Write(raw)
	case bool:
		buf.WriteString(strconv.FormatBool(typed))
	case nil:
		return strconv.ErrSyntax
	default:
		raw, err := json.Marshal(typed)
		if err != nil {
			return err
		}
		buf.Write(raw)
	}
	return nil
}
