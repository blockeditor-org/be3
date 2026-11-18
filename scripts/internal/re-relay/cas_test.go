package main

import (
	"bytes"
	"encoding/base64"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"sort"
	"strings"
	"sync"
	"testing"
	"time"
)

func digest(hash string, size uint64) []byte {
	var d []byte
	d = appendBytesField(d, 1, []byte(hash))
	d = appendVarint(d, 2<<3|0)
	return appendVarint(d, size)
}

func findMissingRequest(digests ...[]byte) []byte {
	request := appendBytesField(nil, 1, []byte("main"))
	for _, d := range digests {
		request = appendBytesField(request, 2, d)
	}
	return append(appendVarint(request, 4<<3|0), 1) // digest_function SHA256
}

func keysOf(t *testing.T, message []byte, number int) []string {
	t.Helper()
	_, digests, err := splitRepeated(message, number)
	if err != nil {
		t.Fatal(err)
	}
	var keys []string
	for _, d := range digests {
		key, err := parseDigest(d)
		if err != nil {
			t.Fatal(err)
		}
		keys = append(keys, string(key))
	}
	sort.Strings(keys)
	return keys
}

// A fake server: it holds `held`, answers FindMissingBlobs with the rest, and records every digest it was asked about.
type fakeServer struct {
	mu    sync.Mutex
	held  map[string]bool
	asked []string
}

func (s *fakeServer) ask(t *testing.T) func([]byte) ([]byte, error) {
	return func(request []byte) ([]byte, error) {
		s.mu.Lock()
		defer s.mu.Unlock()
		var answer []byte
		_, digests, _ := splitRepeated(request, 2)
		for _, d := range digests {
			key, _ := parseDigest(d)
			s.asked = append(s.asked, string(key))
			if !s.held[string(key)] {
				answer = appendBytesField(answer, 1, d)
			}
		}
		return answer, nil
	}
}

func TestFindMissingAnswersFromWhatTheServerSaidItHas(t *testing.T) {
	cache := newCASCache()
	server := &fakeServer{held: map[string]bool{"a/1": true, "b/2": true}}
	answer, err := cache.findMissing(nil, findMissingRequest(digest("a", 1), digest("b", 2), digest("c", 3)), server.ask(t))
	if err != nil {
		t.Fatal(err)
	}
	if got := keysOf(t, answer, 1); strings.Join(got, ",") != "c/3" {
		t.Fatalf("missing %v, want c/3", got)
	}
	// c/3 is uploaded; then nothing about a, b or c reaches the server.
	cache.markPresent(digestKey("c", 3))
	server.asked = nil
	answer, err = cache.findMissing(nil, findMissingRequest(digest("a", 1), digest("c", 3)), server.ask(t))
	if err != nil || len(answer) != 0 || len(server.asked) != 0 {
		t.Fatalf("answer %q, err %v, asked %v: want an empty answer from the cache", answer, err, server.asked)
	}
	// The other fields of the request (its instance name and digest function) go to the server as they were.
	rest, _, _ := splitRepeated(findMissingRequest(digest("d", 4)), 2)
	var forwarded []byte
	cache.findMissing(nil, findMissingRequest(digest("d", 4)), func(request []byte) ([]byte, error) {
		forwarded = request
		return nil, nil
	})
	if got, _, _ := splitRepeated(forwarded, 2); !bytes.Equal(got, rest) {
		t.Fatalf("forwarded %q, want the request's other fields %q kept", got, rest)
	}
}

func TestConcurrentCallersUploadASharedBlobOnce(t *testing.T) {
	cache := newCASCache()
	server := &fakeServer{held: map[string]bool{}}
	first, err := cache.findMissing(nil, findMissingRequest(digest("x", 9)), server.ask(t))
	if err != nil || strings.Join(keysOf(t, first, 1), ",") != "x/9" {
		t.Fatalf("first caller: %v %v, want x/9 missing", keysOf(t, first, 1), err)
	}
	// A second caller asks while the first uploads: it waits, and hears nothing is missing once the upload lands.
	done := make(chan []byte)
	go func() {
		answer, err := cache.findMissing(nil, findMissingRequest(digest("x", 9)), server.ask(t))
		if err != nil {
			t.Error(err)
		}
		done <- answer
	}()
	select {
	case <-done:
		t.Fatal("the second caller did not wait for the upload")
	case <-time.After(50 * time.Millisecond):
	}
	cache.markPresent(digestKey("x", 9))
	if answer := <-done; len(answer) != 0 {
		t.Fatalf("second caller heard %v missing", keysOf(t, answer, 1))
	}
	if strings.Join(server.asked, ",") != "x/9" {
		t.Fatalf("server asked about %v, want x/9 once", server.asked)
	}
}

func TestAWaiterAsksItselfWhenTheUploadFails(t *testing.T) {
	cache := newCASCache()
	server := &fakeServer{held: map[string]bool{}}
	cache.findMissing(nil, findMissingRequest(digest("y", 5)), server.ask(t))
	done := make(chan []byte)
	go func() {
		answer, _ := cache.findMissing(nil, findMissingRequest(digest("y", 5)), server.ask(t))
		done <- answer
	}()
	time.Sleep(20 * time.Millisecond)
	cache.release(digestKey("y", 5)) // the first caller's upload failed
	if got := keysOf(t, <-done, 1); strings.Join(got, ",") != "y/5" {
		t.Fatalf("waiter heard %v missing, want y/5, so that it uploads", got)
	}
}

func TestAPendingUploadIsWaitedForOnlyUntilItsDeadline(t *testing.T) {
	cache := newCASCache()
	now := time.Now()
	cache.now = func() time.Time { return now }
	cache.claim([]blobKey{"z/1"})
	now = now.Add(pendingFor + time.Second)
	ask, waits := cache.claim([]blobKey{"z/1"})
	if len(ask) != 1 || len(waits) != 0 {
		t.Fatalf("ask %v waits %v: an upload past its deadline should be taken over", ask, waits)
	}
}

func TestWhatTheServerHasIsForgottenAfterAWhile(t *testing.T) {
	cache := newCASCache()
	now := time.Now()
	cache.now = func() time.Time { return now }
	cache.markPresent("p/1")
	if ask, _ := cache.claim([]blobKey{"p/1"}); len(ask) != 0 {
		t.Fatal("a blob the server just took was asked about")
	}
	now = now.Add(presentFor + time.Second)
	if ask, _ := cache.claim([]blobKey{"p/1"}); len(ask) != 1 {
		t.Fatal("a blob remembered past presentFor was not asked about again")
	}
}

func TestUploadsAreReadFromTheirRequestsAndAnswers(t *testing.T) {
	var request []byte
	for _, d := range [][]byte{digest("a", 1), digest("b", 2)} {
		entry := appendBytesField(nil, 1, d)
		entry = appendBytesField(entry, 2, []byte("data"))
		request = appendBytesField(request, 2, entry)
	}
	keys, err := batchUpdateKeys(appendBytesField(request, 1, []byte("main")))
	if err != nil || len(keys) != 2 || keys[0] != "a/1" || keys[1] != "b/2" {
		t.Fatalf("keys %v %v", keys, err)
	}
	refusedStatus := append(appendVarint(nil, 1<<3|0), 13)
	answer := appendBytesField(nil, 1, appendBytesField(nil, 1, digest("a", 1)))
	answer = appendBytesField(answer, 1, appendBytesField(appendBytesField(nil, 1, digest("b", 2)), 2, refusedStatus))
	accepted, refused, err := batchUpdateAccepted(answer)
	if err != nil || len(accepted) != 1 || accepted[0] != "a/1" || len(refused) != 1 || refused[0] != "b/2" {
		t.Fatalf("accepted %v refused %v %v", accepted, refused, err)
	}
	for name, want := range map[string]blobKey{
		"main/uploads/1f2e/blobs/abc/12":                    "abc/12",
		"uploads/1f2e/blobs/abc/12":                         "abc/12",
		"main/uploads/1f2e/compressed-blobs/zstd/abc/12":    "abc/12",
		"uploads/1f2e/blobs/abc/12/some/metadata/afterward": "abc/12",
	} {
		if key, _, err := writeKey(appendBytesField(nil, 1, []byte(name))); err != nil || key != want {
			t.Errorf("%s: %q %v, want %q", name, key, err, want)
		}
	}
	if _, _, err := writeKey(appendBytesField(nil, 1, []byte("uploads/x/other/abc"))); err == nil {
		t.Error("a resource name without a blob was read")
	}
}

// The whole path: an HTTP request through handle, with the server's answer in an HTTP response.
func TestHandleAnswersFindMissingBlobsOverGRPC(t *testing.T) {
	cache := newCASCache()
	cache.markPresent("a/1")
	sent := 0
	send := func(body []byte) (*http.Response, string) {
		sent++
		messages, _ := splitMessages(body)
		missing := appendBytesField(nil, 1, digest("c", 3))
		if !bytes.Contains(messages[0], []byte("c")) {
			missing = nil
		}
		return &http.Response{
			StatusCode: 200,
			Header:     http.Header{"Content-Type": {"application/grpc"}},
			Body:       io.NopCloser(bytes.NewReader(frame(missing))),
			Trailer:    http.Header{"Grpc-Status": {"0"}},
		}, ""
	}
	request := func(digests ...[]byte) *httptest.ResponseRecorder {
		w := httptest.NewRecorder()
		r := httptest.NewRequest("POST", findMissingPath, nil)
		if !cache.handle(w, r, frame(findMissingRequest(digests...)), send) {
			t.Fatal("FindMissingBlobs was not handled")
		}
		return w
	}
	w := request(digest("a", 1))
	if sent != 0 || w.Body.Len() != 5 || w.Result().Trailer.Get("Grpc-Status") != "0" {
		t.Fatalf("sent %d, body %q: want an empty answer from the cache", sent, w.Body.Bytes())
	}
	w = request(digest("a", 1), digest("c", 3))
	messages, _ := splitMessages(w.Body.Bytes())
	if sent != 1 || len(messages) != 1 || strings.Join(keysOf(t, messages[0], 1), ",") != "c/3" {
		t.Fatalf("sent %d, answer %q: want c/3 missing, asked once", sent, w.Body.Bytes())
	}
	if cache.handle(httptest.NewRecorder(), httptest.NewRequest("POST", "/build.bazel.remote.execution.v2.Execution/Execute", nil), nil, send) {
		t.Fatal("Execute was taken by the cache")
	}
}

// A google.rpc.Status of FAILED_PRECONDITION whose PreconditionFailure names these blobs MISSING, as NativeLink sends.
func missingStatus(keys ...string) []byte {
	var failure []byte
	for _, key := range keys {
		violation := appendBytesField(nil, 1, []byte("MISSING"))
		violation = appendBytesField(violation, 2, []byte("blobs/"+key))
		failure = appendBytesField(failure, 1, violation)
	}
	detail := appendBytesField(nil, 1, []byte("type.googleapis.com/google.rpc.PreconditionFailure"))
	detail = appendBytesField(detail, 2, failure)
	status := append(appendVarint(nil, 1<<3|0), codeFailedPrecondition)
	status = appendBytesField(status, 2, []byte("Action is missing from CAS; client should re-upload it and retry"))
	if len(keys) > 0 {
		status = appendBytesField(status, 3, detail)
	}
	return status
}

func executeRequest(action []byte) []byte {
	return frame(appendBytesField(appendBytesField(nil, 1, []byte("main")), 6, action))
}

// A transport that answers every call with respond, as the server would.
type answering func(r *http.Request) *http.Response

func (f answering) RoundTrip(r *http.Request) (*http.Response, error) { return f(r), nil }

func grpcResponse(header, trailer http.Header, messages ...[]byte) *http.Response {
	header.Set("Content-Type", "application/grpc")
	var body []byte
	for _, message := range messages {
		body = append(body, frame(message)...)
	}
	return &http.Response{StatusCode: 200, Header: header, Body: io.NopCloser(bytes.NewReader(body)), Trailer: trailer}
}

// A cache that remembers the server holding these, and a log of what it says.
func cacheHolding(keys ...blobKey) (*casCache, *[]string) {
	cache := newCASCache()
	var log []string
	cache.logf = func(format string, args ...any) { log = append(log, fmt.Sprintf(format, args...)) }
	cache.markPresent(keys...)
	return cache, &log
}

func through(cache *casCache, path string, body []byte, respond answering) *httptest.ResponseRecorder {
	w := httptest.NewRecorder()
	relay(respond, "http", "server", cache, w, httptest.NewRequest("POST", path, bytes.NewReader(body)))
	return w
}

// After the answer, a FindMissingBlobs about every blob reaches the server, which says which it lacks.
func askedAfter(t *testing.T, cache *casCache, keys ...string) []string {
	t.Helper()
	server := &fakeServer{held: map[string]bool{}}
	var digests [][]byte
	for _, key := range keys {
		hash, size, _ := strings.Cut(key, "/")
		var n uint64
		fmt.Sscan(size, &n)
		digests = append(digests, digest(hash, n))
	}
	if _, err := cache.findMissing(nil, findMissingRequest(digests...), server.ask(t)); err != nil {
		t.Fatal(err)
	}
	sort.Strings(server.asked)
	return server.asked
}

func TestAnExecuteRefusedForAMissingActionForgetsIt(t *testing.T) {
	cache, log := cacheHolding("act/142", "in/7", "other/3")
	status := missingStatus("act/142", "in/7")
	w := through(cache, executePath, executeRequest(digest("act", 142)), func(r *http.Request) *http.Response {
		return grpcResponse(http.Header{
			"Grpc-Status":             {"9"},
			"Grpc-Message":            {"Action act-142 is missing from CAS; client should re-upload it and retry"},
			"Grpc-Status-Details-Bin": {base64.RawStdEncoding.EncodeToString(status)},
		}, nil)
	})
	if w.Result().Header.Get("Grpc-Status") != "9" {
		t.Fatalf("buck2 was not told FAILED_PRECONDITION: %v", w.Result().Header)
	}
	if got := strings.Join(askedAfter(t, cache, "act/142", "in/7", "other/3"), ","); got != "act/142,in/7" {
		t.Fatalf("the server was asked about %s, want act/142,in/7 (and other/3 answered from the cache)", got)
	}
	if len(*log) != 1 || !strings.Contains((*log)[0], "act/142") || !strings.Contains((*log)[0], "in/7") ||
		!strings.Contains((*log)[0], "FAILED_PRECONDITION") {
		t.Fatalf("log %q: want one line naming both blobs and why", *log)
	}
}

func TestAnExecuteRefusedWithoutNamingBlobsForgetsAll(t *testing.T) {
	cache, log := cacheHolding("a/1", "b/2")
	through(cache, executePath, executeRequest(digest("act", 142)), func(r *http.Request) *http.Response {
		return grpcResponse(http.Header{}, http.Header{"Grpc-Status": {"9"}, "Grpc-Message": {"missing"}},
			appendBytesField(nil, 1, []byte("operations/1")))
	})
	if got := strings.Join(askedAfter(t, cache, "a/1", "b/2"), ","); got != "a/1,b/2" {
		t.Fatalf("the server was asked about %s, want a/1,b/2", got)
	}
	if len(*log) != 1 || !strings.Contains((*log)[0], "forgot all 2") {
		t.Fatalf("log %q", *log)
	}
}

// A worker that found an input missing answers in the Operation: done, its ExecuteResponse's status
// FAILED_PRECONDITION, and the call itself OK.
func TestAnOperationThatFoundAnInputMissingForgetsIt(t *testing.T) {
	cache, _ := cacheHolding("dir/40", "act/142", "other/3")
	executeResponse := appendBytesField(nil, 3, missingStatus("dir/40"))
	any := appendBytesField(appendBytesField(nil, 1, []byte("type.googleapis.com/build.bazel.remote.execution.v2.ExecuteResponse")), 2, executeResponse)
	operation := appendBytesField(nil, 1, []byte("operations/1"))
	operation = append(appendVarint(operation, 3<<3|0), 1)
	operation = appendBytesField(operation, 5, any)
	w := through(cache, executePath, executeRequest(digest("act", 142)), func(r *http.Request) *http.Response {
		return grpcResponse(http.Header{}, http.Header{"Grpc-Status": {"0"}}, operation)
	})
	if messages, _ := splitMessages(w.Body.Bytes()); len(messages) != 1 || !bytes.Equal(messages[0], operation) {
		t.Fatalf("the Operation was not passed on as it was: %q", w.Body.Bytes())
	}
	if got := strings.Join(askedAfter(t, cache, "dir/40", "act/142", "other/3"), ","); got != "act/142,dir/40" {
		t.Fatalf("the server was asked about %s, want act/142,dir/40", got)
	}
}

func TestAReadOfABlobTheServerLacksForgetsIt(t *testing.T) {
	cache, _ := cacheHolding("out/9", "other/3")
	request := frame(appendBytesField(nil, 1, []byte("main/blobs/out/9")))
	through(cache, readPath, request, func(r *http.Request) *http.Response {
		return grpcResponse(http.Header{"Grpc-Status": {"5"}, "Grpc-Message": {"not found"}}, nil)
	})
	batch := appendBytesField(nil, 1, appendBytesField(appendBytesField(nil, 1, digest("other", 3)), 3, append(appendVarint(nil, 1<<3|0), codeNotFound)))
	through(cache, batchReadPath, frame(appendBytesField(nil, 2, digest("other", 3))), func(r *http.Request) *http.Response {
		return grpcResponse(http.Header{}, http.Header{"Grpc-Status": {"0"}}, batch)
	})
	if got := strings.Join(askedAfter(t, cache, "out/9", "other/3"), ","); got != "other/3,out/9" {
		t.Fatalf("the server was asked about %s, want both", got)
	}
}

// Answers that do not say a blob is missing leave the cache as it was.
func TestOtherFailuresForgetNothing(t *testing.T) {
	cache, log := cacheHolding("a/1")
	through(cache, executePath, executeRequest(digest("act", 142)), func(r *http.Request) *http.Response {
		return grpcResponse(http.Header{"Grpc-Status": {"14"}, "Grpc-Message": {"unavailable"}}, nil)
	})
	through(cache, "/build.bazel.remote.execution.v2.ActionCache/GetActionResult", frame(nil), func(r *http.Request) *http.Response {
		return grpcResponse(http.Header{"Grpc-Status": {"5"}}, nil)
	})
	if got := askedAfter(t, cache, "a/1"); len(got) != 0 || len(*log) != 0 {
		t.Fatalf("asked %v, log %q: want nothing forgotten", got, *log)
	}
}

// An upload only marks a blob present when the server says it took all of it.
func TestOnlyWholeUploadsAreRemembered(t *testing.T) {
	cache, _ := cacheHolding()
	write := func(name string, committed int64, data string) bool {
		request := appendBytesField(nil, 1, []byte(name))
		request = append(appendVarint(request, 3<<3|0), 1)
		request = appendBytesField(request, 4, []byte(data))
		answer := appendVarint(appendVarint(nil, 1<<3|0), uint64(committed))
		w := httptest.NewRecorder()
		cache.handle(w, httptest.NewRequest("POST", writePath, nil), frame(request), func([]byte) (*http.Response, string) {
			return grpcResponse(http.Header{}, http.Header{"Grpc-Status": {"0"}}, answer), ""
		})
		key, _, _ := writeKey(request)
		cache.mu.Lock()
		defer cache.mu.Unlock()
		return cache.isPresent(key)
	}
	if !write("uploads/u/blobs/whole/4", 4, "abcd") {
		t.Error("a whole Write was not remembered")
	}
	if write("uploads/u/blobs/short/4", 2, "abcd") {
		t.Error("a Write the server committed two bytes of was remembered")
	}
	if !write("uploads/u/compressed-blobs/zstd/z/100", 3, "xyz") || !write("uploads/u/compressed-blobs/zstd/y/100", -1, "x") {
		t.Error("a whole compressed Write was not remembered")
	}
	if write("uploads/u/compressed-blobs/zstd/x/100", 1, "xyz") {
		t.Error("a short compressed Write was remembered")
	}

	// BatchUpdateBlobs: a blob refused, or left out of the answer, is not remembered.
	var request []byte
	for _, key := range []string{"a", "b", "c"} {
		request = appendBytesField(request, 2, appendBytesField(appendBytesField(nil, 1, digest(key, 1)), 2, []byte("x")))
	}
	answer := appendBytesField(nil, 1, appendBytesField(nil, 1, digest("a", 1)))
	answer = appendBytesField(answer, 1, appendBytesField(appendBytesField(nil, 1, digest("b", 1)), 2, append(appendVarint(nil, 1<<3|0), 13)))
	cache.claim([]blobKey{"c/1"})
	cache.handle(httptest.NewRecorder(), httptest.NewRequest("POST", batchUpdatePath, nil), frame(request), func([]byte) (*http.Response, string) {
		return grpcResponse(http.Header{}, http.Header{"Grpc-Status": {"0"}}, answer), ""
	})
	if got := strings.Join(askedAfter(t, cache, "a/1", "b/1", "c/1"), ","); got != "b/1,c/1" {
		t.Fatalf("asked about %s, want b/1,c/1: c/1 left out of the answer must not hold its waiters back either", got)
	}
}
