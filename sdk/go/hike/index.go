package hike

import (
	"encoding/hex"
	"unicode/utf8"
)

// HeaderRead is one successful section 9 index body.
// Time is nil when header.time is absent or not a Unix second in range.
// Hash and height were still read.
type HeaderRead struct {
	Natural []byte
	Height  uint64
	Time    *uint64
	Network string
}

// ReadIndexDocument applies section 9. A nil result is an unsuccessful read.
func ReadIndexDocument(body string) *HeaderRead {
	if len(body) == 0 || hasBOM(body) {
		return nil
	}
	v, rest, err := parseJSON(body, 0)
	if err != nil {
		return nil
	}
	if skipSpace(rest) != len(rest) {
		return nil
	}
	obj, ok := v.(jmap)
	if !ok {
		return nil
	}
	header, ok := obj.get("header").(jmap)
	if !ok {
		return nil
	}
	spelling, ok := header.get("hash").(string)
	if !ok || len(spelling) != 64 {
		return nil
	}
	display, err := hex.DecodeString(spelling)
	if err != nil || len(display) != 32 {
		return nil
	}
	height, ok := header.get("height").(jint)
	if !ok || height.neg || height.u > JSONIntMax {
		return nil
	}
	natural := make([]byte, 32)
	for i := 0; i < 32; i++ {
		natural[i] = display[31-i]
	}
	out := &HeaderRead{Natural: natural, Height: height.u}
	if net, ok := obj.get("network").(string); ok {
		out.Network = net
	}
	if raw, present := header.lookup("time"); present {
		t, ok := raw.(jint)
		if ok && !t.neg && t.u <= HeaderTimeMax {
			tv := t.u
			out.Time = &tv
		}
	}
	return out
}

func hasBOM(body string) bool {
	return len(body) >= 3 && body[0] == 0xEF && body[1] == 0xBB && body[2] == 0xBF
}

type jint struct {
	u   uint64
	neg bool
}

type jmap struct {
	keys []string
	vals []any
}

func (m jmap) get(key string) any {
	v, _ := m.lookup(key)
	return v
}

func (m jmap) lookup(key string) (any, bool) {
	for i, k := range m.keys {
		if k == key {
			return m.vals[i], true
		}
	}
	return nil, false
}

func parseJSON(s string, i int) (any, string, error) {
	i = skipSpace(s[i:]) + i
	if i >= len(s) {
		return nil, s, errCbor
	}
	switch s[i] {
	case '{':
		return parseObject(s, i)
	case '[':
		return parseArray(s, i)
	case '"':
		str, next, err := parseString(s, i)
		return str, s[next:], err
	case 't':
		if hasPrefixAt(s, i, "true") {
			return true, s[i+4:], nil
		}
	case 'f':
		if hasPrefixAt(s, i, "false") {
			return false, s[i+5:], nil
		}
	case 'n':
		if hasPrefixAt(s, i, "null") {
			return nil, s[i+4:], nil
		}
	default:
		if s[i] == '-' || (s[i] >= '0' && s[i] <= '9') {
			return parseNumber(s, i)
		}
	}
	return nil, s, errCbor
}

func parseObject(s string, i int) (jmap, string, error) {
	i++ // '{'
	var m jmap
	i = skipSpace(s[i:]) + i
	if i < len(s) && s[i] == '}' {
		return m, s[i+1:], nil
	}
	for {
		i = skipSpace(s[i:]) + i
		if i >= len(s) || s[i] != '"' {
			return jmap{}, s, errCbor
		}
		key, next, err := parseString(s, i)
		if err != nil {
			return jmap{}, s, err
		}
		i = skipSpace(s[next:]) + next
		if i >= len(s) || s[i] != ':' {
			return jmap{}, s, errCbor
		}
		i++
		val, rest, err := parseJSON(s, i)
		if err != nil {
			return jmap{}, rest, err
		}
		if _, exists := m.lookup(key); exists {
			return jmap{}, rest, errCbor
		}
		m.keys = append(m.keys, key)
		m.vals = append(m.vals, val)
		i = skipSpace(rest)
		// rest may be the remainder; skipSpace works on a string, so recompute from rest.
		rest = rest[skipSpace(rest):]
		if rest == "" {
			return jmap{}, rest, errCbor
		}
		if rest[0] == '}' {
			return m, rest[1:], nil
		}
		if rest[0] != ',' {
			return jmap{}, rest, errCbor
		}
		s = rest[1:]
		i = 0
	}
}

func parseArray(s string, i int) ([]any, string, error) {
	i++
	var arr []any
	i = skipSpace(s[i:]) + i
	if i < len(s) && s[i] == ']' {
		return arr, s[i+1:], nil
	}
	for {
		val, rest, err := parseJSON(s, i)
		if err != nil {
			return nil, rest, err
		}
		arr = append(arr, val)
		rest = rest[skipSpace(rest):]
		if rest == "" {
			return nil, rest, errCbor
		}
		if rest[0] == ']' {
			return arr, rest[1:], nil
		}
		if rest[0] != ',' {
			return nil, rest, errCbor
		}
		s = rest[1:]
		i = 0
	}
}

func parseString(s string, i int) (string, int, error) {
	if i >= len(s) || s[i] != '"' {
		return "", i, errCbor
	}
	i++
	var b []byte
	for i < len(s) {
		c := s[i]
		if c == '"' {
			if !utf8.Valid(b) {
				return "", i, errCbor
			}
			return string(b), i + 1, nil
		}
		if c == '\\' {
			if i+1 >= len(s) {
				return "", i, errCbor
			}
			switch s[i+1] {
			case '"', '\\', '/':
				b = append(b, s[i+1])
				i += 2
			case 'b':
				b = append(b, '\b')
				i += 2
			case 'f':
				b = append(b, '\f')
				i += 2
			case 'n':
				b = append(b, '\n')
				i += 2
			case 'r':
				b = append(b, '\r')
				i += 2
			case 't':
				b = append(b, '\t')
				i += 2
			case 'u':
				if i+6 > len(s) {
					return "", i, errCbor
				}
				r, err := hexRune(s[i+2 : i+6])
				if err != nil {
					return "", i, err
				}
				b = utf8.AppendRune(b, r)
				i += 6
			default:
				return "", i, errCbor
			}
			continue
		}
		if c < 0x20 {
			return "", i, errCbor
		}
		b = append(b, c)
		i++
	}
	return "", i, errCbor
}

func hexRune(s string) (rune, error) {
	var v rune
	for i := 0; i < 4; i++ {
		c := s[i]
		var d rune
		switch {
		case c >= '0' && c <= '9':
			d = rune(c - '0')
		case c >= 'a' && c <= 'f':
			d = rune(c-'a') + 10
		case c >= 'A' && c <= 'F':
			d = rune(c-'A') + 10
		default:
			return 0, errCbor
		}
		v = v<<4 | d
	}
	return v, nil
}

func parseNumber(s string, i int) (any, string, error) {
	start := i
	neg := false
	if s[i] == '-' {
		neg = true
		i++
	}
	if i >= len(s) || s[i] < '0' || s[i] > '9' {
		return nil, s, errCbor
	}
	if s[i] == '0' && i+1 < len(s) && s[i+1] >= '0' && s[i+1] <= '9' {
		return nil, s, errCbor
	}
	for i < len(s) && s[i] >= '0' && s[i] <= '9' {
		i++
	}
	intEnd := i
	if i < len(s) && (s[i] == '.' || s[i] == 'e' || s[i] == 'E') {
		if s[i] == '.' {
			i++
			if i >= len(s) || s[i] < '0' || s[i] > '9' {
				return nil, s, errCbor
			}
			for i < len(s) && s[i] >= '0' && s[i] <= '9' {
				i++
			}
		}
		if i < len(s) && (s[i] == 'e' || s[i] == 'E') {
			i++
			if i < len(s) && (s[i] == '+' || s[i] == '-') {
				i++
			}
			if i >= len(s) || s[i] < '0' || s[i] > '9' {
				return nil, s, errCbor
			}
			for i < len(s) && s[i] >= '0' && s[i] <= '9' {
				i++
			}
		}
		return s[start:i], s[i:], nil
	}
	digits := s[start:intEnd]
	if neg {
		digits = s[start+1 : intEnd]
	}
	var u uint64
	for _, c := range digits {
		u = u*10 + uint64(c-'0')
	}
	return jint{u: u, neg: neg}, s[intEnd:], nil
}

func skipSpace(s string) int {
	i := 0
	for i < len(s) {
		switch s[i] {
		case ' ', '\n', '\r', '\t':
			i++
		default:
			return i
		}
	}
	return i
}

func hasPrefixAt(s string, i int, prefix string) bool {
	return i+len(prefix) <= len(s) && s[i:i+len(prefix)] == prefix
}
