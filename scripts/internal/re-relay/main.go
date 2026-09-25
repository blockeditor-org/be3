// A relay between Bazel and BuildBuddy for a machine whose way out is an
// HTTPS proxy. Bazel's remote execution and build event clients dial their
// gRPC endpoints directly and never read HTTPS_PROXY, so where the proxy is the
// only way out, or the one that adds credentials, Bazel is pointed at this instead:
// it takes gRPC in over cleartext HTTP/2 on localhost and sends each call on
// through the proxy. A proxy that only speaks HTTP/1.1 drops gRPC's
// trailers, so a call that ends without them is given the status the proxy
// left out: the one in its headers when it had nothing else to say, and OK
// when its body arrived whole.
//
//	re-relay serve  -listen 127.0.0.1:18980 -upstream remote.buildbuddy.io
//	re-relay ensure -listen 127.0.0.1:18980 -upstream remote.buildbuddy.io -version <hash> -log <path>
//
// ensure returns once a relay of that version is listening, starting one in
// the background if there is none, and replacing one of another version.
package main

import (
	"bufio"
	"bytes"
	"context"
	"flag"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"strconv"
	"strings"
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
	listen := flags.String("listen", "127.0.0.1:18980", "the address Bazel is pointed at")
	upstream := flags.String("upstream", "remote.buildbuddy.io", "the host the calls go on to, over HTTPS on 443")
	version := flags.String("version", "", "what ensure expects a running relay to answer")
	logPath := flags.String("log", "", "where a relay ensure starts writes its errors")
	connections := flags.Int("connections", 128, "at most this many connections to the upstream at once")
	flags.Parse(os.Args[2:])
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
		Proxy:              http.ProxyFromEnvironment,
		ForceAttemptHTTP2:  true,
		MaxConnsPerHost:    connections,
		MaxIdleConns:       connections,
		IdleConnTimeout:    90 * time.Second,
		DisableCompression: true,
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
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		relay(transport, upstream, w, r)
	})

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

// The proxy between here and BuildBuddy sometimes answers a call itself, with
// an HTTP error and a page of text, when it could not reach the upstream.
// That is not gRPC, and passed on as it was it read to the client as a corrupt
// message and failed the whole build. Every call Bazel makes to remote
// execution is safe to make again - reads, cache lookups, uploads of content
// named by its hash, and Execute, which runs the action again at worst - so
// each request body is read whole first, and a call that fails before any of
// its answer has been passed on is made again. The one call that streams both
// ways, the build event stream, is sent whole too: Bazel sends every event
// before it waits for the acknowledgements. One that still fails is
// UNAVAILABLE, which Bazel knows how to report.
const attempts = 5

func relay(transport *http.Transport, upstream string, w http.ResponseWriter, r *http.Request) {
	body, err := io.ReadAll(r.Body)
	if err != nil {
		grpcError(w, 14, "re-relay: reading the request: "+err.Error())
		return
	}

	var resp *http.Response
	var failure string
	for attempt := 1; ; attempt++ {
		resp, failure = roundTrip(transport, upstream, r, body)
		if failure == "" {
			break
		}
		fmt.Fprintf(os.Stderr, "re-relay: %s %s, attempt %d of %d: %s\n",
			time.Now().Format(time.RFC3339), r.URL.Path, attempt, attempts, failure)
		if attempt == attempts {
			grpcError(w, 14, "re-relay: "+failure)
			return
		}
		select {
		case <-r.Context().Done():
			return
		case <-time.After(time.Duration(250<<(attempt-1)) * time.Millisecond):
		}
	}
	defer resp.Body.Close()

	copyHeaders(w.Header(), resp.Header)
	trailersOnly := resp.Header.Get("Grpc-Status") != ""
	w.WriteHeader(resp.StatusCode)

	flusher := w.(http.Flusher)
	buffer := make([]byte, 64*1024)
	for {
		n, readErr := resp.Body.Read(buffer)
		if n > 0 {
			if _, err := w.Write(buffer[:n]); err != nil {
				return
			}
			flusher.Flush()
		}
		if readErr == io.EOF {
			break
		}
		if readErr != nil {
			if !trailersOnly {
				w.Header().Set(http.TrailerPrefix+"Grpc-Status", "14")
				w.Header().Set(http.TrailerPrefix+"Grpc-Message", "re-relay: "+readErr.Error())
			}
			return
		}
	}
	if trailersOnly {
		return
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

// One attempt at a call: the response when it is gRPC's, or why it is not.
func roundTrip(transport *http.Transport, upstream string, r *http.Request, body []byte) (*http.Response, string) {
	out, err := http.NewRequestWithContext(r.Context(), r.Method, "https://"+upstream+r.URL.RequestURI(), bytes.NewReader(body))
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
