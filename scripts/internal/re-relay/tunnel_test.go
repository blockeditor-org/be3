package main

import (
	"bytes"
	"context"
	"crypto/rand"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/http/httptest"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

// A tunnel-server in front of upstream, and the base address a client gives dialTunnel.
func startTunnel(t *testing.T, upstream string) string {
	t.Helper()
	server := &tunnelServer{upstream: upstream, sessions: map[string]*tunnelSession{}}
	front := httptest.NewServer(server.handler())
	t.Cleanup(front.Close)
	return front.URL + tunnelPath
}

func TestTunnelCarriesAStreamAcrossPostsAndGetsThatEnd(t *testing.T) {
	oldDown, oldUp, oldStatus := downLife, upLife, statusEvery
	downLife, upLife, statusEvery = 20*time.Millisecond, 30*time.Millisecond, 10*time.Millisecond
	t.Cleanup(func() { downLife, upLife, statusEvery = oldDown, oldUp, oldStatus })

	echo, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { echo.Close() })
	go func() {
		for {
			conn, err := echo.Accept()
			if err != nil {
				return
			}
			go func() { io.Copy(conn, conn); conn.Close() }()
		}
	}()

	base := startTunnel(t, echo.Addr().String())
	conn, err := dialTunnel(context.Background(), http.DefaultTransport, base, new(atomic.Pointer[string]))
	if err != nil {
		t.Fatal(err)
	}
	defer conn.Close()

	want := make([]byte, 24<<20)
	rand.Read(want)
	go func() {
		for sent := 0; sent < len(want); {
			n := min(len(want)-sent, 100_000)
			if _, err := conn.Write(want[sent : sent+n]); err != nil {
				return
			}
			sent += n
		}
	}()
	got := make([]byte, len(want))
	if _, err := io.ReadFull(conn, got); err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(got, want) {
		t.Fatal("the stream came back different")
	}
}

func TestTunnelMultiplexesHTTP2Calls(t *testing.T) {
	h2c := &http.Server{Handler: http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.ProtoMajor != 2 {
			http.Error(w, "not HTTP/2", http.StatusBadRequest)
			return
		}
		body, _ := io.ReadAll(r.Body)
		w.Write(append([]byte(r.URL.Path+":"), body...))
	})}
	h2c.Protocols = new(http.Protocols)
	h2c.Protocols.SetUnencryptedHTTP2(true)
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	go h2c.Serve(listener)
	t.Cleanup(func() { h2c.Close() })

	var requests atomic.Int64
	server := &tunnelServer{upstream: listener.Addr().String(), sessions: map[string]*tunnelSession{}}
	handler := server.handler()
	front := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		requests.Add(1)
		handler.ServeHTTP(w, r)
	}))
	t.Cleanup(front.Close)

	transport := tunnelTransportTo(front.URL+tunnelPath, http.DefaultTransport, new(atomic.Pointer[string]))
	t.Cleanup(transport.(*http.Transport).CloseIdleConnections)
	client := &http.Client{Transport: transport}
	const calls = 500
	var wait sync.WaitGroup
	errs := make(chan error, calls)
	for i := range calls {
		wait.Add(1)
		go func() {
			defer wait.Done()
			path := fmt.Sprintf("/call/%d", i)
			resp, err := client.Post("http://build.server"+path, "text/plain", bytes.NewReader([]byte("body")))
			if err != nil {
				errs <- err
				return
			}
			defer resp.Body.Close()
			got, _ := io.ReadAll(resp.Body)
			if string(got) != path+":body" {
				errs <- fmt.Errorf("%s answered %q", path, got)
			}
		}()
	}
	wait.Wait()
	close(errs)
	for err := range errs {
		t.Fatal(err)
	}
	if n := requests.Load(); n >= calls {
		t.Fatalf("%d calls took %d requests to the tunnel-server", calls, n)
	}
}
