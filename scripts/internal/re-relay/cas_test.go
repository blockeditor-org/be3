package main

import (
	"bytes"
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
		if key, err := writeKey(appendBytesField(nil, 1, []byte(name))); err != nil || key != want {
			t.Errorf("%s: %q %v, want %q", name, key, err, want)
		}
	}
	if _, err := writeKey(appendBytesField(nil, 1, []byte("uploads/x/other/abc"))); err == nil {
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
