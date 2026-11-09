package main

// What the build server already holds, so a slow link carries each blob once and asks about it once.
//
// buck2 asks FindMissingBlobs for every action's inputs and uploads what is missing, and actions that share inputs
// (every action of a crate reads its sources) ask at the same moment, all hear "missing" and all upload the same
// blobs. Over a link with a quarter of a second between question and answer that was most of a cold build's time:
// a crate's 2 MB of sources went up once per action. So the relay that carries those calls keeps:
//
//   - the blobs the server said it has, or accepted, answering FindMissingBlobs about them itself; and
//   - the blobs some caller was just told are missing, which it is uploading: another caller asking about one waits
//     for that upload instead of hearing "missing" and sending it again.
//
// A blob is remembered for presentFor, after which the server is asked again, in case it lost it. A caller that never
// uploads what it was told is missing (it failed, or did not need it) holds the others back for at most pendingFor.

import (
	"bufio"
	"encoding/binary"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"strings"
	"sync"
	"time"
)

const (
	findMissingPath = "/build.bazel.remote.execution.v2.ContentAddressableStorage/FindMissingBlobs"
	batchUpdatePath = "/build.bazel.remote.execution.v2.ContentAddressableStorage/BatchUpdateBlobs"
	writePath       = "/google.bytestream.ByteStream/Write"
	presentFor      = 10 * time.Minute
	pendingFor      = 5 * time.Minute
)

// A digest as the cache keys it: "hash/size".
type blobKey string

func digestKey(hash string, size uint64) blobKey {
	return blobKey(hash + "/" + strconv.FormatUint(size, 10))
}

type casCache struct {
	mu      sync.Mutex
	now     func() time.Time
	present map[blobKey]time.Time // until when the server is taken to hold it
	pending map[blobKey]*upload
}

// A blob some caller was told is missing: done is closed when its upload ends, either way, and nobody waits for it
// past until.
type upload struct {
	done  chan struct{}
	until time.Time
}

func newCASCache() *casCache {
	return &casCache{now: time.Now, present: map[blobKey]time.Time{}, pending: map[blobKey]*upload{}}
}

func (c *casCache) isPresent(key blobKey) bool {
	until, ok := c.present[key]
	if ok && c.now().After(until) {
		delete(c.present, key)
		return false
	}
	return ok
}

// The server holds these: remember them, and release whoever waits on their upload.
func (c *casCache) markPresent(keys ...blobKey) {
	c.mu.Lock()
	defer c.mu.Unlock()
	until := c.now().Add(presentFor)
	for _, key := range keys {
		c.present[key] = until
		c.end(key)
	}
}

func (c *casCache) end(key blobKey) {
	if pending, ok := c.pending[key]; ok {
		close(pending.done)
		delete(c.pending, key)
	}
}

// Nobody is uploading these after all: whoever waits on them asks the server itself.
func (c *casCache) release(keys ...blobKey) {
	c.mu.Lock()
	defer c.mu.Unlock()
	for _, key := range keys {
		c.end(key)
	}
}

// Sorts what a caller asks about: what the server is known to hold, what to ask it (each now pending on this caller),
// and the uploads others are making that the caller waits for first.
func (c *casCache) claim(keys []blobKey) (ask []blobKey, waits map[blobKey]*upload) {
	c.mu.Lock()
	defer c.mu.Unlock()
	now := c.now()
	waits = map[blobKey]*upload{}
	for _, key := range keys {
		if c.isPresent(key) {
			continue
		}
		if pending, ok := c.pending[key]; ok {
			if now.Before(pending.until) {
				waits[key] = pending
				continue
			}
			c.end(key) // its uploader never finished: this caller asks instead
		}
		c.pending[key] = &upload{done: make(chan struct{}), until: now.Add(pendingFor)}
		ask = append(ask, key)
	}
	return ask, waits
}

// Waits for the uploads others are making; returns the ones that did not end with the server holding the blob, for
// the caller to ask about itself.
func (c *casCache) await(stop <-chan struct{}, waits map[blobKey]*upload) ([]blobKey, error) {
	var again []blobKey
	for key, pending := range waits {
		timer := time.NewTimer(time.Until(pending.until))
		select {
		case <-pending.done:
		case <-timer.C:
		case <-stop:
			timer.Stop()
			return nil, errors.New("the call was cancelled")
		}
		timer.Stop()
		c.mu.Lock()
		held := c.isPresent(key)
		c.mu.Unlock()
		if !held {
			again = append(again, key)
		}
	}
	return again, nil
}

// The relay's answer to FindMissingBlobs: from the cache where it can, the server for the rest. A caller told a blob
// is missing is expected to upload it; others asking meanwhile wait for that.
func (c *casCache) findMissing(stop <-chan struct{}, request []byte, ask func([]byte) ([]byte, error)) ([]byte, error) {
	fields, digests, err := splitRepeated(request, 2)
	if err != nil {
		return nil, err
	}
	byKey := map[blobKey][]byte{}
	var keys []blobKey
	for _, digest := range digests {
		key, err := parseDigest(digest)
		if err != nil {
			return nil, err
		}
		if _, seen := byKey[key]; !seen {
			keys = append(keys, key)
		}
		byKey[key] = digest
	}
	unknown, waits := c.claim(keys)
	for len(waits) > 0 {
		// Uploads by others: once each ends, a blob it did not leave on the server is asked about again.
		again, err := c.await(stop, waits)
		if err != nil {
			c.release(unknown...)
			return nil, err
		}
		var more []blobKey
		more, waits = c.claim(again)
		unknown = append(unknown, more...)
	}
	if len(unknown) == 0 {
		return nil, nil
	}
	forwarded := append([]byte(nil), fields...)
	for _, key := range unknown {
		forwarded = appendBytesField(forwarded, 2, byKey[key])
	}
	answer, err := ask(forwarded)
	if err != nil {
		c.release(unknown...)
		return nil, err
	}
	_, missingDigests, err := splitRepeated(answer, 1)
	if err != nil {
		c.release(unknown...)
		return nil, err
	}
	missing := map[blobKey]bool{}
	for _, digest := range missingDigests {
		key, err := parseDigest(digest)
		if err != nil {
			c.release(unknown...)
			return nil, err
		}
		missing[key] = true
	}
	var held []blobKey
	for _, key := range unknown {
		if !missing[key] {
			held = append(held, key)
		}
	}
	c.markPresent(held...)
	return answer, nil
}

// What a BatchUpdateBlobs request uploads, and what its answer says the server took.
func batchUpdateKeys(request []byte) ([]blobKey, error) {
	_, requests, err := splitRepeated(request, 2)
	if err != nil {
		return nil, err
	}
	keys := make([]blobKey, 0, len(requests))
	for _, entry := range requests {
		digest, err := firstBytesField(entry, 1)
		if err != nil {
			return nil, err
		}
		key, err := parseDigest(digest)
		if err != nil {
			return nil, err
		}
		keys = append(keys, key)
	}
	return keys, nil
}

func batchUpdateAccepted(answer []byte) (accepted, refused []blobKey, err error) {
	_, responses, err := splitRepeated(answer, 1)
	if err != nil {
		return nil, nil, err
	}
	for _, entry := range responses {
		digest, err := firstBytesField(entry, 1)
		if err != nil {
			return nil, nil, err
		}
		key, err := parseDigest(digest)
		if err != nil {
			return nil, nil, err
		}
		status, _ := firstBytesField(entry, 2)
		code, _ := firstVarintField(status, 1)
		if code == 0 {
			accepted = append(accepted, key)
		} else {
			refused = append(refused, key)
		}
	}
	return accepted, refused, nil
}

// The blob a ByteStream Write uploads, from its first message's resource name:
// [INSTANCE/]uploads/UUID/blobs/HASH/SIZE, or .../compressed-blobs/COMPRESSOR/HASH/SIZE.
func writeKey(firstMessage []byte) (blobKey, error) {
	name, err := firstBytesField(firstMessage, 1)
	if err != nil {
		return "", err
	}
	parts := strings.Split(string(name), "/")
	for i, part := range parts {
		hashAt := -1
		if part == "blobs" {
			hashAt = i + 1
		} else if part == "compressed-blobs" {
			hashAt = i + 2
		}
		if hashAt > 0 && hashAt+1 < len(parts) {
			size, err := strconv.ParseUint(parts[hashAt+1], 10, 64)
			if err != nil {
				return "", fmt.Errorf("a resource name with a bad size: %q", name)
			}
			return digestKey(parts[hashAt], size), nil
		}
	}
	return "", fmt.Errorf("a resource name without a blob: %q", name)
}

// Handles the calls the cache takes part in, and reports whether it did; any other call is the relay's to make.
func (c *casCache) handle(w http.ResponseWriter, r *http.Request, body []byte, send func([]byte) (*http.Response, string)) bool {
	switch r.URL.Path {
	case findMissingPath, batchUpdatePath, writePath:
	default:
		return false
	}
	messages, err := splitMessages(body)
	if err != nil || len(messages) == 0 {
		return false // not one this understands: passed on as it is
	}
	switch r.URL.Path {
	case findMissingPath:
		if len(messages) != 1 {
			return false
		}
		var status string
		answer, err := c.findMissing(r.Context().Done(), messages[0], func(request []byte) ([]byte, error) {
			reply, code, failure := unary(send, frame(request))
			if failure != "" {
				return nil, errors.New(failure)
			}
			if code != "0" {
				status = code
				return nil, fmt.Errorf("the server answered status %s", code)
			}
			return reply, nil
		})
		if err != nil {
			code := 14
			if status != "" {
				code, _ = strconv.Atoi(status)
			}
			grpcError(w, code, "re-relay: "+err.Error())
			return true
		}
		writeUnary(w, answer)
	case batchUpdatePath:
		keys, err := batchUpdateKeys(messages[0])
		if err != nil || len(messages) != 1 {
			return false
		}
		reply, code, failure := unary(send, body)
		if failure != "" || code != "0" {
			c.release(keys...)
			passFailure(w, code, failure)
			return true
		}
		accepted, refused, err := batchUpdateAccepted(reply)
		if err != nil {
			c.release(keys...)
		} else {
			c.markPresent(accepted...)
			c.release(refused...)
		}
		writeUnary(w, reply)
	case writePath:
		key, err := writeKey(messages[0])
		if err != nil {
			return false
		}
		reply, code, failure := unary(send, body)
		if failure != "" || code != "0" {
			c.release(key)
			passFailure(w, code, failure)
			return true
		}
		c.markPresent(key)
		writeUnary(w, reply)
	}
	return true
}

// A unary call's one answer, its status, or why there was none.
func unary(send func([]byte) (*http.Response, string), body []byte) ([]byte, string, string) {
	resp, failure := send(body)
	if failure != "" {
		return nil, "", failure
	}
	defer resp.Body.Close()
	if status := resp.Header.Get("Grpc-Status"); status != "" {
		return nil, status, resp.Header.Get("Grpc-Message")
	}
	message, err := readMessage(bufio.NewReader(resp.Body))
	if err != nil {
		return nil, "", "reading the answer: " + err.Error()
	}
	io.Copy(io.Discard, resp.Body)
	if message[0] != 0 {
		return nil, "", "a compressed answer"
	}
	status := resp.Trailer.Get("Grpc-Status")
	if status == "" {
		status = "0" // a proxy that speaks HTTP/1.1 drops trailers; the answer came whole
	} else if status != "0" {
		return nil, status, resp.Trailer.Get("Grpc-Message")
	}
	return message[5:], status, ""
}

func passFailure(w http.ResponseWriter, code, message string) {
	if code == "" {
		grpcError(w, 14, "re-relay: "+message)
		return
	}
	number, _ := strconv.Atoi(code)
	grpcError(w, number, message)
}

func writeUnary(w http.ResponseWriter, message []byte) {
	w.Header().Set("Content-Type", "application/grpc")
	w.WriteHeader(http.StatusOK)
	w.Write(frame(message))
	w.Header().Set(http.TrailerPrefix+"Grpc-Status", "0")
}

// gRPC framing: a flag byte (0, not compressed), the length in four bytes, the message.
func frame(message []byte) []byte {
	out := make([]byte, 5, 5+len(message))
	binary.BigEndian.PutUint32(out[1:], uint32(len(message)))
	return append(out, message...)
}

// The messages of a request body; nil for a compressed one, which the cache leaves alone.
func splitMessages(body []byte) ([][]byte, error) {
	var messages [][]byte
	for len(body) > 0 {
		if len(body) < 5 {
			return nil, errors.New("a cut message")
		}
		if body[0] != 0 {
			return nil, nil
		}
		n := int(binary.BigEndian.Uint32(body[1:5]))
		if len(body) < 5+n {
			return nil, errors.New("a cut message")
		}
		messages = append(messages, body[5:5+n])
		body = body[5+n:]
	}
	return messages, nil
}

// Protocol buffers, just enough of them: a message is a run of fields, each a key (number and wire type) and a value.

func readVarint(b []byte) (uint64, int, error) {
	var value uint64
	for i := 0; i < len(b) && i < 10; i++ {
		value |= uint64(b[i]&0x7f) << (7 * i)
		if b[i] < 0x80 {
			return value, i + 1, nil
		}
	}
	return 0, 0, errors.New("a bad varint")
}

func appendVarint(b []byte, v uint64) []byte {
	for v >= 0x80 {
		b = append(b, byte(v)|0x80)
		v >>= 7
	}
	return append(b, byte(v))
}

func appendBytesField(b []byte, number int, value []byte) []byte {
	b = appendVarint(b, uint64(number)<<3|2)
	b = appendVarint(b, uint64(len(value)))
	return append(b, value...)
}

// Calls each for every field: its number, wire type, raw bytes (key included), and value (the bytes of a
// length-delimited field, or the number of a varint).
func eachField(message []byte, each func(number int, wireType int, raw []byte, value []byte, varint uint64) error) error {
	for at := 0; at < len(message); {
		start := at
		key, n, err := readVarint(message[at:])
		if err != nil {
			return err
		}
		at += n
		number, wireType := int(key>>3), int(key&7)
		var value []byte
		var varint uint64
		switch wireType {
		case 0:
			if varint, n, err = readVarint(message[at:]); err != nil {
				return err
			}
			at += n
		case 1:
			at += 8
		case 2:
			length, n, err := readVarint(message[at:])
			if err != nil {
				return err
			}
			at += n
			if uint64(len(message)-at) < length {
				return errors.New("a cut field")
			}
			value = message[at : at+int(length)]
			at += int(length)
		case 5:
			at += 4
		default:
			return fmt.Errorf("wire type %d", wireType)
		}
		if at > len(message) {
			return errors.New("a cut field")
		}
		if err := each(number, wireType, message[start:at], value, varint); err != nil {
			return err
		}
	}
	return nil
}

// A message's repeated length-delimited field `number`, apart from its other fields, kept as they were.
func splitRepeated(message []byte, number int) (rest []byte, values [][]byte, err error) {
	err = eachField(message, func(n, wireType int, raw, value []byte, _ uint64) error {
		if n == number && wireType == 2 {
			values = append(values, value)
		} else {
			rest = append(rest, raw...)
		}
		return nil
	})
	return rest, values, err
}

func firstBytesField(message []byte, number int) ([]byte, error) {
	var found []byte
	ok := false
	err := eachField(message, func(n, wireType int, _, value []byte, _ uint64) error {
		if !ok && n == number && wireType == 2 {
			found, ok = value, true
		}
		return nil
	})
	if err == nil && !ok {
		err = fmt.Errorf("no field %d", number)
	}
	return found, err
}

func firstVarintField(message []byte, number int) (uint64, error) {
	var found uint64
	err := eachField(message, func(n, wireType int, _, _ []byte, varint uint64) error {
		if n == number && wireType == 0 {
			found = varint
		}
		return nil
	})
	return found, err
}

// A Digest: hash (1) and size_bytes (2).
func parseDigest(digest []byte) (blobKey, error) {
	hash, err := firstBytesField(digest, 1)
	if err != nil {
		return "", err
	}
	size, err := firstVarintField(digest, 2)
	if err != nil {
		return "", err
	}
	return digestKey(string(hash), size), nil
}
