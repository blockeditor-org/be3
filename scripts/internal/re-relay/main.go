// A relay between buck2 and the build server for a machine whose way out is an
// HTTPS proxy. buck2's remote execution client dials its gRPC endpoints
// directly and never reads HTTPS_PROXY, so where the proxy is the only way
// out, or the one that adds credentials, buck2 is pointed at this instead:
// it takes gRPC in over cleartext HTTP/2 on localhost and sends each call on
// through the proxy. A proxy that only speaks HTTP/1.1 drops gRPC's
// trailers, so a call that ends without them is given the status the proxy
// left out: the one in its headers when it had nothing else to say, and OK
// when its body arrived whole.
//
//	re-relay serve  -listen 127.0.0.1:18980 -upstream reapih-p5cvniii97l8a.iad4.namespaced.app
//	re-relay ensure -listen 127.0.0.1:18980 -upstream reapih-p5cvniii97l8a.iad4.namespaced.app -version <hash> -log <path>
//
// ensure returns once a relay of that version is listening, starting one in
// the background if there is none, and replacing one of another version.
//
// GET /__be3-re-relay/calls lists the calls in flight and what each waits on,
// then every goroutine's stack (guides/build_server.md).
package main

import (
	"bufio"
	"bytes"
	"context"
	"encoding/binary"
	"flag"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/http/httptrace"
	"os"
	"os/exec"
	"runtime/pprof"
	"slices"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"
)

const controlPath = "/__be3-re-relay"

var hopByHop = map[string]bool{
	"Connection":        true,
	"Keep-Alive":        true,
	"Proxy-Connection":  true,
	"Transfer-Encoding": true,
	"Upgrade":           true,
	"Content-Length":    true,
	"Trailer":           true,
}

func main() {
	if len(os.Args) < 2 {
		fail("usage: re-relay serve|ensure [flags]")
	}
	flags := flag.NewFlagSet(os.Args[1], flag.ExitOnError)
	listen := flags.String("listen", "127.0.0.1:18980", "the address buck2 is pointed at")
	upstream := flags.String("upstream", "", "the host the calls go on to, over HTTPS on 443")
	version := flags.String("version", "", "what ensure expects a running relay to answer")
	logPath := flags.String("log", "", "where a relay ensure starts writes its errors")
	connections := flags.Int("connections", 128, "at most this many connections to the upstream at once")
	flags.Parse(os.Args[2:])
	if *upstream == "" {
		fail("-upstream is required")
	}
	switch os.Args[1] {
	case "serve":
		serve(*listen, *upstream, *version, *connections)
	case "ensure":
		ensure(*listen, *upstream, *version, *logPath, *connections)
	default:
		fail("usage: re-relay serve|ensure [flags]")
	}
}

func fail(format string, args ...any) {
	fmt.Fprintf(os.Stderr, "re-relay: "+format+"\n", args...)
	os.Exit(1)
}

func ensure(listen, upstream, version, logPath string, connections int) {
	local := &http.Client{Transport: &http.Transport{Proxy: nil}, Timeout: 5 * time.Second}
	base := "http://" + listen + controlPath
	resp, err := local.Get(base)
	if err == nil {
		body, _ := io.ReadAll(resp.Body)
		resp.Body.Close()
		if resp.StatusCode != http.StatusOK || !strings.HasPrefix(string(body), "be3-re-relay ") {
			fail("%s is taken by something other than the relay", listen)
		}
		if strings.TrimPrefix(string(body), "be3-re-relay ") == version {
			return
		}
		if resp, err := local.Post(base+"/quit", "text/plain", nil); err == nil {
			resp.Body.Close()
		}
	}

	self, err := os.Executable()
	if err != nil {
		fail("%v", err)
	}
	command := exec.Command(self, "serve", "-listen", listen, "-upstream", upstream,
		"-version", version, "-connections", strconv.Itoa(connections))
	detach(command)
	if logPath != "" {
		log, err := os.OpenFile(logPath, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
		if err != nil {
			fail("%v", err)
		}
		command.Stderr = log
	}
	ready, err := command.StdoutPipe()
	if err != nil {
		fail("%v", err)
	}
	if err := command.Start(); err != nil {
		fail("%v", err)
	}
	line, _ := bufio.NewReader(ready).ReadString('\n')
	if strings.TrimSpace(line) != "ready" {
		fail("the relay did not start; %s says why", logPath)
	}
}

func serve(listen, upstream, version string, connections int) {
	transport := &http.Transport{
		Proxy:               http.ProxyFromEnvironment,
		DialContext:         (&net.Dialer{Timeout: 30 * time.Second, KeepAlive: 30 * time.Second}).DialContext,
		TLSHandshakeTimeout: 30 * time.Second,
		ForceAttemptHTTP2:   true,
		MaxConnsPerHost:     connections,
		MaxIdleConns:        connections,
		MaxIdleConnsPerHost: connections,
		IdleConnTimeout:     90 * time.Second,
		DisableCompression:  true,
	}

	listener, err := net.Listen("tcp", listen)
	if err != nil {
		fail("%v", err)
	}

	mux := http.NewServeMux()
	server := &http.Server{Handler: mux}
	mux.HandleFunc("GET "+controlPath, func(w http.ResponseWriter, r *http.Request) {
		fmt.Fprintf(w, "be3-re-relay %s", version)
	})
	mux.HandleFunc("POST "+controlPath+"/quit", func(w http.ResponseWriter, r *http.Request) {
		listener.Close()
		w.WriteHeader(http.StatusOK)
		go func() {
			server.Shutdown(context.Background())
			os.Exit(0)
		}()
	})
	mux.HandleFunc("GET "+controlPath+"/calls", func(w http.ResponseWriter, r *http.Request) {
		describeCalls(w)
		fmt.Fprintln(w)
		pprof.Lookup("goroutine").WriteTo(w, 2)
	})
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		relay(transport, upstream, w, r)
	})
	go reportStalls()

	server.Protocols = new(http.Protocols)
	server.Protocols.SetHTTP1(true)
	server.Protocols.SetUnencryptedHTTP2(true)

	fmt.Println("ready")
	os.Stdout.Close()
	if err := server.Serve(listener); err != nil && err != http.ErrServerClosed && !isClosed(err) {
		fail("%v", err)
	}
	select {}
}

func isClosed(err error) bool {
	return strings.Contains(err.Error(), "use of closed network connection")
}

// The proxy between here and the build server sometimes answers a call itself, with
// an HTTP error and a page of text, when it could not reach the upstream.
// That is not gRPC, and passed on as it was it read to buck2 as a corrupt
// message and failed the whole build. Every call buck2 makes to remote
// execution is safe to make again - reads, cache lookups, uploads of content
// named by its hash, and Execute, which runs the action again at worst - and
// none streams in both directions, so each request body is read whole first,
// and a call that fails before any of its answer has been passed on is made
// again. One that still fails is UNAVAILABLE, which buck2 knows how to report.
const attempts = 5

// The proxy also cuts every call five minutes after it starts, while the
// action an Execute asked for runs on. So when an Execute's answer breaks off,
// the same Execute is made again: the server joins it to the action already
// running, or answers from the cache once that has finished, and buck2 reads
// the rest of the answer on the call it made. Each message in the answer is a
// whole account of the action, so a repeated one is harmless; only whole
// messages are passed on, so a cut never leaves half of one.
const resumes = 24

var resumable = map[string]bool{
	"/build.bazel.remote.execution.v2.Execution/Execute":       true,
	"/build.bazel.remote.execution.v2.Execution/WaitExecution": true,
}

func relay(transport *http.Transport, upstream string, w http.ResponseWriter, r *http.Request) {
	c := begin(r.URL.Path)
	defer c.end()
	body, err := io.ReadAll(r.Body)
	if err != nil {
		grpcError(w, 14, "re-relay: reading the request: "+err.Error())
		return
	}

	resp, failure := call(c, transport, upstream, r, body)
	if failure != "" {
		grpcError(w, 14, "re-relay: "+failure)
		return
	}
	defer func() { resp.Body.Close() }()

	copyHeaders(w.Header(), resp.Header)
	if resp.Header.Get("Grpc-Status") != "" {
		w.WriteHeader(resp.StatusCode)
		return
	}
	w.WriteHeader(resp.StatusCode)

	flusher := w.(http.Flusher)
	answer := bufio.NewReaderSize(resp.Body, 64*1024)
	for resumed := 0; ; {
		c.set(phaseUpstream)
		message, readErr := readMessage(answer)
		if message != nil {
			c.set(phaseBuck2)
			if _, err := w.Write(message); err != nil {
				return
			}
			flusher.Flush()
			c.sent(len(message))
		}
		if readErr == io.EOF {
			break
		}
		if readErr == nil {
			continue
		}
		if !resumable[r.URL.Path] || resumed == resumes || r.Context().Err() != nil {
			trailerError(w, "re-relay: "+readErr.Error())
			return
		}
		resumed++
		fmt.Fprintf(os.Stderr, "re-relay: %s %s broke off (%v), making it again (%d of %d)\n",
			time.Now().Format(time.RFC3339), r.URL.Path, readErr, resumed, resumes)
		resp.Body.Close()
		resp, failure = call(c, transport, upstream, r, body)
		if failure != "" {
			resp = &http.Response{Body: http.NoBody}
			trailerError(w, "re-relay: "+failure)
			return
		}
		if status := resp.Header.Get("Grpc-Status"); status != "" {
			w.Header().Set(http.TrailerPrefix+"Grpc-Status", status)
			w.Header().Set(http.TrailerPrefix+"Grpc-Message", resp.Header.Get("Grpc-Message"))
			return
		}
		answer = bufio.NewReaderSize(resp.Body, 64*1024)
	}
	for k, vs := range resp.Trailer {
		for _, v := range vs {
			w.Header().Add(http.TrailerPrefix+k, v)
		}
	}
	if resp.Trailer.Get("Grpc-Status") == "" {
		w.Header().Set(http.TrailerPrefix+"Grpc-Status", "0")
	}
}

// A call made until it answers with gRPC, or why it never did.
func call(c *inFlight, transport *http.Transport, upstream string, r *http.Request, body []byte) (*http.Response, string) {
	for attempt := 1; ; attempt++ {
		resp, failure := roundTrip(c, transport, upstream, r, body)
		if failure == "" {
			return resp, ""
		}
		fmt.Fprintf(os.Stderr, "re-relay: %s %s, attempt %d of %d: %s\n",
			time.Now().Format(time.RFC3339), r.URL.Path, attempt, attempts, failure)
		if attempt == attempts {
			return nil, failure
		}
		select {
		case <-r.Context().Done():
			return nil, r.Context().Err().Error()
		case <-time.After(time.Duration(250<<(attempt-1)) * time.Millisecond):
		}
	}
}

// One gRPC message: a flag byte, its length in four, and that many bytes. A
// message cut short is an error, and none of it is returned.
func readMessage(answer *bufio.Reader) ([]byte, error) {
	header := make([]byte, 5)
	if _, err := io.ReadFull(answer, header); err != nil {
		return nil, err
	}
	message := make([]byte, 5+int(binary.BigEndian.Uint32(header[1:])))
	copy(message, header)
	if _, err := io.ReadFull(answer, message[5:]); err != nil {
		if err == io.EOF {
			err = io.ErrUnexpectedEOF
		}
		return nil, err
	}
	return message, nil
}

func trailerError(w http.ResponseWriter, message string) {
	w.Header().Set(http.TrailerPrefix+"Grpc-Status", "14")
	w.Header().Set(http.TrailerPrefix+"Grpc-Message", message)
}

// One attempt at a call: the response when it is gRPC's, or why it is not.
func roundTrip(c *inFlight, transport *http.Transport, upstream string, r *http.Request, body []byte) (*http.Response, string) {
	c.set(phaseConnection)
	trace := &httptrace.ClientTrace{GotConn: func(httptrace.GotConnInfo) { c.set(phaseUpstream) }}
	out, err := http.NewRequestWithContext(httptrace.WithClientTrace(r.Context(), trace), r.Method, "https://"+upstream+r.URL.RequestURI(), bytes.NewReader(body))
	if err != nil {
		return nil, err.Error()
	}
	out.ContentLength = -1
	copyHeaders(out.Header, r.Header)

	resp, err := transport.RoundTrip(out)
	if err != nil {
		return nil, err.Error()
	}
	if resp.StatusCode == http.StatusOK && strings.HasPrefix(resp.Header.Get("Content-Type"), "application/grpc") {
		return resp, ""
	}
	page, _ := io.ReadAll(io.LimitReader(resp.Body, 512))
	resp.Body.Close()
	return nil, fmt.Sprintf("the proxy answered %s: %s", resp.Status, strings.TrimSpace(string(page)))
}

func copyHeaders(to, from http.Header) {
	for k, vs := range from {
		if hopByHop[k] {
			continue
		}
		for _, v := range vs {
			to.Add(k, v)
		}
	}
}

func grpcError(w http.ResponseWriter, code int, message string) {
	w.Header().Set("Content-Type", "application/grpc")
	w.Header().Set("Grpc-Status", strconv.Itoa(code))
	w.Header().Set("Grpc-Message", message)
	w.WriteHeader(http.StatusOK)
}

// What a call in flight waits on. A build that stops with calls waiting on
// buck2 is buck2 no longer reading its answers; one with calls waiting on the
// upstream is the proxy or the build server; and calls waiting for a
// connection are queued behind -connections others.
const (
	phaseConnection int32 = iota
	phaseUpstream
	phaseBuck2
)

var phaseNames = [...]string{
	phaseConnection: "waiting for a connection to the upstream",
	phaseUpstream:   "waiting on the upstream",
	phaseBuck2:      "waiting on buck2 to read",
}

type inFlight struct {
	path    string
	started time.Time
	phase   atomic.Int32
	since   atomic.Int64 // when it last moved, in Unix nanoseconds
	bytes   atomic.Int64 // passed on to buck2
}

var calls sync.Map // *inFlight -> struct{}

func begin(path string) *inFlight {
	c := &inFlight{path: path, started: time.Now()}
	c.since.Store(c.started.UnixNano())
	calls.Store(c, struct{}{})
	return c
}

func (c *inFlight) end() { calls.Delete(c) }

func (c *inFlight) set(phase int32) {
	if c.phase.Swap(phase) != phase {
		c.since.Store(time.Now().UnixNano())
	}
}

func (c *inFlight) sent(n int) {
	c.bytes.Add(int64(n))
	c.since.Store(time.Now().UnixNano())
}

func (c *inFlight) idle(now time.Time) time.Duration {
	return now.Sub(time.Unix(0, c.since.Load()))
}

// An Execute waits on the upstream for as long as its action runs, so that
// wait alone is not a stall.
func (c *inFlight) stalled(now time.Time) bool {
	if c.idle(now) < stallAfter {
		return false
	}
	return c.phase.Load() != phaseUpstream || !resumable[c.path]
}

const stallAfter = time.Minute

func snapshot() []*inFlight {
	var all []*inFlight
	calls.Range(func(key, _ any) bool {
		all = append(all, key.(*inFlight))
		return true
	})
	slices.SortFunc(all, func(a, b *inFlight) int { return a.started.Compare(b.started) })
	return all
}

// Every 30 seconds while any call has not moved for a minute, one line in the
// log says how many wait on what, so the log of a stopped build says which
// side stopped.
func reportStalls() {
	for range time.Tick(30 * time.Second) {
		now := time.Now()
		all := snapshot()
		var count [len(phaseNames)]int
		var oldest [len(phaseNames)]time.Duration
		stalled := 0
		for _, c := range all {
			if !c.stalled(now) {
				continue
			}
			stalled++
			phase := c.phase.Load()
			count[phase]++
			oldest[phase] = max(oldest[phase], c.idle(now))
		}
		if stalled == 0 {
			continue
		}
		var parts []string
		for phase, n := range count {
			if n > 0 {
				parts = append(parts, fmt.Sprintf("%d %s (longest %s)", n, phaseNames[phase], oldest[phase].Round(time.Second)))
			}
		}
		fmt.Fprintf(os.Stderr, "re-relay: %s %d of %d calls have not moved for %s: %s\n",
			now.Format(time.RFC3339), stalled, len(all), stallAfter, strings.Join(parts, ", "))
	}
}

func describeCalls(w io.Writer) {
	now := time.Now()
	all := snapshot()
	fmt.Fprintf(w, "%d calls in flight\n", len(all))
	for _, c := range all {
		fmt.Fprintf(w, "%8s old, %8s %s, %d bytes passed on, %s\n",
			now.Sub(c.started).Round(time.Second), c.idle(now).Round(time.Second),
			phaseNames[c.phase.Load()], c.bytes.Load(), c.path)
	}
}
