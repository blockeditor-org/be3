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
//	re-relay serve  -listen 127.0.0.1:18980 -upstream blocks.pfg.pw
//	re-relay ensure -listen 127.0.0.1:18980 -upstream blocks.pfg.pw -version <hash> -log <path>
//
// ensure returns once a relay of that version is listening, starting one in
// the background if there is none, and replacing one of another version.
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

func relay(transport *http.Transport, upstream string, w http.ResponseWriter, r *http.Request) {
	body, err := io.ReadAll(r.Body)
	if err != nil {
		grpcError(w, 14, "re-relay: reading the request: "+err.Error())
		return
	}

	resp, failure := call(transport, upstream, r, r.URL.Path, body)
	if failure != "" {
		grpcError(w, 14, "re-relay: "+failure)
		return
	}
	defer resp.Body.Close()

	if r.URL.Path == executionService+"Execute" || r.URL.Path == executionService+"WaitExecution" {
		relayOperations(transport, upstream, w, r, resp)
		return
	}

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

// A call to path, made again until it succeeds or has failed attempts times:
// its response, or why the last attempt failed.
func call(transport *http.Transport, upstream string, r *http.Request, path string, body []byte) (*http.Response, string) {
	for attempt := 1; ; attempt++ {
		resp, failure := roundTrip(transport, upstream, r, path, body)
		if failure == "" {
			return resp, ""
		}
		fmt.Fprintf(os.Stderr, "re-relay: %s %s, attempt %d of %d: %s\n",
			time.Now().Format(time.RFC3339), path, attempt, attempts, failure)
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

const executionService = "/build.bazel.remote.execution.v2.Execution/"

// Execute and WaitExecution answer with a stream of Operations that stays
// open for as long as the action runs. The proxy closes a connection after
// about five minutes however busy it is, which failed every longer action.
// Here the stream is passed on an Operation at a time, and when it breaks
// before the last one, it is picked up again with WaitExecution on the name
// of the last Operation passed on.
func relayOperations(transport *http.Transport, upstream string, w http.ResponseWriter, r *http.Request, resp *http.Response) {
	copyHeaders(w.Header(), resp.Header)
	w.WriteHeader(resp.StatusCode)
	if resp.Header.Get("Grpc-Status") != "" {
		return
	}

	flusher := w.(http.Flusher)
	name := ""
	for {
		frames := bufio.NewReader(resp.Body)
		done := false
		var broken error
		for {
			frame, err := readFrame(frames)
			if err != nil {
				if err != io.EOF {
					broken = err
				}
				break
			}
			if operationName, operationDone, ok := parseOperation(frame[5:]); ok {
				name = operationName
				done = done || operationDone
			}
			if _, err := w.Write(frame); err != nil {
				return
			}
			flusher.Flush()
		}
		if broken == nil && (done || resp.Trailer.Get("Grpc-Status") != "") {
			resp.Body.Close()
			for k, vs := range resp.Trailer {
				for _, v := range vs {
					w.Header().Add(http.TrailerPrefix+k, v)
				}
			}
			if resp.Trailer.Get("Grpc-Status") == "" {
				w.Header().Set(http.TrailerPrefix+"Grpc-Status", "0")
			}
			return
		}
		resp.Body.Close()
		if r.Context().Err() != nil {
			return
		}

		why := "the stream ended before the action did"
		if broken != nil {
			why = broken.Error()
		}
		if name == "" {
			w.Header().Set(http.TrailerPrefix+"Grpc-Status", "14")
			w.Header().Set(http.TrailerPrefix+"Grpc-Message", "re-relay: "+why)
			return
		}
		fmt.Fprintf(os.Stderr, "re-relay: %s %s: %s; waiting on %s again\n",
			time.Now().Format(time.RFC3339), r.URL.Path, why, name)

		var failure string
		resp, failure = call(transport, upstream, r, executionService+"WaitExecution", waitExecutionRequest(name))
		if failure != "" {
			w.Header().Set(http.TrailerPrefix+"Grpc-Status", "14")
			w.Header().Set(http.TrailerPrefix+"Grpc-Message", "re-relay: "+failure)
			return
		}
		if status := resp.Header.Get("Grpc-Status"); status != "" {
			resp.Body.Close()
			w.Header().Set(http.TrailerPrefix+"Grpc-Status", status)
			w.Header().Set(http.TrailerPrefix+"Grpc-Message", resp.Header.Get("Grpc-Message"))
			return
		}
	}
}

// One gRPC message with its five byte prefix: a compression flag and the
// message's length.
func readFrame(r *bufio.Reader) ([]byte, error) {
	prefix := make([]byte, 5)
	if _, err := io.ReadFull(r, prefix); err != nil {
		return nil, err
	}
	length := binary.BigEndian.Uint32(prefix[1:])
	if length > 1<<30 {
		return nil, fmt.Errorf("a message of %d bytes", length)
	}
	frame := make([]byte, 5+int(length))
	copy(frame, prefix)
	if _, err := io.ReadFull(r, frame[5:]); err != nil {
		if err == io.EOF {
			err = io.ErrUnexpectedEOF
		}
		return nil, err
	}
	return frame, nil
}

// The name (field 1) and done (field 3) of a google.longrunning.Operation.
func parseOperation(message []byte) (string, bool, bool) {
	name, done := "", false
	for len(message) > 0 {
		tag, n := binary.Uvarint(message)
		if n <= 0 {
			return "", false, false
		}
		message = message[n:]
		switch tag & 7 {
		case 0:
			value, n := binary.Uvarint(message)
			if n <= 0 {
				return "", false, false
			}
			message = message[n:]
			if tag>>3 == 3 {
				done = value != 0
			}
		case 1:
			if len(message) < 8 {
				return "", false, false
			}
			message = message[8:]
		case 2:
			length, n := binary.Uvarint(message)
			if n <= 0 || uint64(len(message)-n) < length {
				return "", false, false
			}
			if tag>>3 == 1 {
				name = string(message[n : n+int(length)])
			}
			message = message[n+int(length):]
		case 5:
			if len(message) < 4 {
				return "", false, false
			}
			message = message[4:]
		default:
			return "", false, false
		}
	}
	return name, done, name != ""
}

// A WaitExecutionRequest naming the operation, as one gRPC message.
func waitExecutionRequest(name string) []byte {
	message := binary.AppendUvarint([]byte{1<<3 | 2}, uint64(len(name)))
	message = append(message, name...)
	frame := binary.BigEndian.AppendUint32([]byte{0}, uint32(len(message)))
	return append(frame, message...)
}

// One attempt at a call: the response when it is gRPC's, or why it is not.
func roundTrip(transport *http.Transport, upstream string, r *http.Request, path string, body []byte) (*http.Response, string) {
	out, err := http.NewRequestWithContext(r.Context(), r.Method, "https://"+upstream+path, bytes.NewReader(body))
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
