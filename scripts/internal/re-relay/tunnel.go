// The tunnel: one byte stream between the relay and the build server carried by two long HTTP/1.1 requests, so that
// HTTP/2 can run inside it and multiplex every call on it. A Claude Code cloud session's HTTPS proxy speaks only
// HTTP/1.1, handles about 240 requests a second however many connections carry them, and adds about 80 ms to each
// request on its way up; a cache-warm ./scripts/verify makes about 24,000 calls, so each call as its own request spent
// about 100 s in that limit. Through the tunnel the proxy sees one long POST carrying everything the relay sends and
// one long GET carrying everything the server sends, and it passes both along as they come.
//
// The client side is a net.Conn (tunnelConn) that an HTTP/2 transport dials. Its bytes go up a chunked POST to
// /__be3-tunnel/stream, which says the stream offset it starts at; the server writes what it has not written yet and
// skips the rest. The server's bytes come down a GET to /__be3-tunnel/down, which streams from the offset the client
// asks for. Each ends after upLife or downLife to stay clear of the proxy's five-minute cut, and when one ends or
// breaks the client starts another from where the server, or it, got to. While anything is unconfirmed the client asks
// /__be3-tunnel/status about once a second how far the server got, saying how much it has received, so each side
// can forget what the other has.
//
// The server side (tunnel-server, behind Caddy, which checks the token) joins each session to one TCP connection to
// NativeLink's cleartext HTTP/2 port.
package main

import (
	"bufio"
	"bytes"
	"context"
	"crypto/rand"
	"crypto/tls"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"slices"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"
)

const tunnelPath = "/__be3-tunnel"

const (
	// The most the relay has written and the server has not confirmed before Write waits, and the most the client
	// holds unread or the server keeps sent and unconfirmed before each stops reading what feeds it.
	upBuffer   = 64 << 20
	downBuffer = 64 << 20
	// Requests that fail in a row before the tunnel gives up.
	tunnelAttempts = 8
)

// How long the client streams one POST and the server one GET before ending it, under the proxy's five-minute cut,
// and how often the client asks how far the server got while anything is unconfirmed. Variables for the tests.
var (
	upLife      = 4 * time.Minute
	downLife    = 4 * time.Minute
	statusEvery = time.Second
)

// ---- The client ----

// An HTTP/2 transport whose every connection is a tunnel to upstream's tunnel-server, through the environment's
// proxy. authorization is the Authorization header the tunnel's requests carry, empty for a proxy that adds it.
func tunnelTransport(upstream string, authorization *atomic.Pointer[string]) http.RoundTripper {
	outer := &http.Transport{
		Proxy:               http.ProxyFromEnvironment,
		MaxIdleConnsPerHost: 8,
		IdleConnTimeout:     90 * time.Second,
		DisableCompression:  true,
		TLSNextProto:        map[string]func(string, *tls.Conn) http.RoundTripper{},
	}
	return tunnelTransportTo("https://"+upstream+tunnelPath, outer, authorization)
}

func tunnelTransportTo(base string, outer http.RoundTripper, authorization *atomic.Pointer[string]) http.RoundTripper {
	inner := &http.Transport{
		// One tunnel carries every call: HTTP/2 multiplexes them on it.
		MaxConnsPerHost:    1,
		DisableCompression: true,
		DialContext: func(ctx context.Context, _, _ string) (net.Conn, error) {
			return dialTunnel(ctx, outer, base, authorization)
		},
		HTTP2: &http.HTTP2Config{
			// Enough window for a fast download across an ocean.
			MaxReceiveBufferPerConnection: 32 << 20,
			MaxReceiveBufferPerStream:     16 << 20,
			// A tunnel that stops answering is replaced rather than waited on.
			SendPingTimeout: 30 * time.Second,
			PingTimeout:     60 * time.Second,
		},
	}
	inner.Protocols = new(http.Protocols)
	inner.Protocols.SetUnencryptedHTTP2(true)
	return inner
}

type tunnelConn struct {
	client        *http.Client
	base, session string
	authorization *atomic.Pointer[string]
	ctx           context.Context
	cancel        context.CancelFunc

	mu        sync.Mutex
	changed   *sync.Cond
	up        []byte // written and not confirmed by the server, from stream offset upBase
	upBase    int64
	sent      int64  // the offset the POST streaming now got to
	down      []byte // received and not yet read
	received  int64  // bytes received in all
	confirmed int64  // received as of the last status the server was given
	err       error  // why the tunnel ended
	stats     tunnelStats
}

// What a tunnel did since it last said, for re-relay's log.
type tunnelStats struct {
	posts, gets, statuses int
	sent                  int64
}

func dialTunnel(ctx context.Context, transport http.RoundTripper, base string, authorization *atomic.Pointer[string]) (net.Conn, error) {
	id := make([]byte, 16)
	if _, err := rand.Read(id); err != nil {
		return nil, err
	}
	c := &tunnelConn{
		client:        &http.Client{Transport: transport},
		base:          base,
		session:       hex.EncodeToString(id),
		authorization: authorization,
	}
	c.changed = sync.NewCond(&c.mu)
	c.ctx, c.cancel = context.WithCancel(context.Background())
	if _, err := c.ask(ctx, http.MethodPost, "open", 0); err != nil {
		c.cancel()
		return nil, fmt.Errorf("opening the tunnel: %w", err)
	}
	go c.upload()
	go c.receive()
	go c.confirm()
	go c.report()
	return c, nil
}

// Writes what the tunnel did to re-relay's log every 30 s that it did anything.
func (c *tunnelConn) report() {
	ticker := time.NewTicker(30 * time.Second)
	defer ticker.Stop()
	for {
		select {
		case <-c.ctx.Done():
			return
		case <-ticker.C:
		}
		c.mu.Lock()
		s := c.stats
		c.stats = tunnelStats{}
		received := c.received
		c.mu.Unlock()
		if s.sent == 0 && s.posts == 0 && s.gets == 0 {
			continue
		}
		fmt.Fprintf(os.Stderr, "re-relay: %s tunnel %s: %d bytes sent, %d received in all; %d POSTs, %d GETs and %d status checks started\n",
			time.Now().Format(time.RFC3339), c.session[:8], s.sent, received, s.posts, s.gets, s.statuses)
	}
}

// A request to the tunnel-server that should be made again when it fails, and why it is not worth making again.
type permanent struct{ error }

func (c *tunnelConn) request(ctx context.Context, method, what string, offset int64, body io.Reader) (*http.Request, error) {
	c.mu.Lock()
	received := c.received
	c.mu.Unlock()
	url := fmt.Sprintf("%s/%s?s=%s&off=%d&ack=%d", c.base, what, c.session, offset, received)
	r, err := http.NewRequestWithContext(ctx, method, url, body)
	if err != nil {
		return nil, err
	}
	if a := c.authorization.Load(); a != nil && *a != "" {
		r.Header.Set("Authorization", *a)
	}
	r.Header.Set("Content-Type", "application/octet-stream")
	return r, nil
}

func statusError(resp *http.Response) error {
	page, _ := io.ReadAll(io.LimitReader(resp.Body, 512))
	err := fmt.Errorf("the tunnel answered %s: %s", resp.Status, bytes.TrimSpace(page))
	switch resp.StatusCode {
	case http.StatusGone, http.StatusNotFound, http.StatusUnauthorized, http.StatusForbidden, http.StatusConflict:
		return permanent{err}
	}
	return err
}

func backOff(ctx context.Context, failures int) {
	select {
	case <-ctx.Done():
	case <-time.After(time.Duration(100<<min(failures, 5)) * time.Millisecond):
	}
}

// One short request, made again until it is answered or the tunnel ends: its answer.
func (c *tunnelConn) ask(ctx context.Context, method, what string, offset int64) ([]byte, error) {
	var last error
	for attempt := 1; attempt <= tunnelAttempts; attempt++ {
		r, err := c.request(ctx, method, what, offset, nil)
		if err != nil {
			return nil, err
		}
		resp, err := c.client.Do(r)
		if err == nil {
			if resp.StatusCode == http.StatusOK {
				body, err := io.ReadAll(resp.Body)
				resp.Body.Close()
				if err == nil {
					return body, nil
				}
			} else {
				err = statusError(resp)
				resp.Body.Close()
				if errors.As(err, new(permanent)) {
					return nil, err
				}
			}
		}
		last = err
		if ctx.Err() != nil {
			return nil, ctx.Err()
		}
		backOff(ctx, attempt)
	}
	return nil, last
}

func (c *tunnelConn) fail(err error) {
	c.mu.Lock()
	if c.err == nil {
		c.err = err
	}
	c.changed.Broadcast()
	c.mu.Unlock()
	c.cancel()
}

// Asks how far the server got, telling it how much has been received, and forgets what it confirms.
func (c *tunnelConn) status() error {
	c.mu.Lock()
	c.stats.statuses++
	received := c.received
	c.mu.Unlock()
	answer, err := c.ask(c.ctx, http.MethodGet, "status", 0)
	if err != nil {
		return err
	}
	next, err := strconv.ParseInt(strings.TrimSpace(string(answer)), 10, 64)
	if err != nil {
		return permanent{fmt.Errorf("the tunnel's status was %q", answer)}
	}
	c.mu.Lock()
	defer c.mu.Unlock()
	if next > c.upBase && next <= c.upBase+int64(len(c.up)) {
		c.up = append(c.up[:0:0], c.up[next-c.upBase:]...)
		c.upBase = next
		c.changed.Broadcast()
	}
	c.confirmed = max(c.confirmed, received)
	return nil
}

// Asks how far the server got every statusEvery while anything sent or received is unconfirmed.
func (c *tunnelConn) confirm() {
	ticker := time.NewTicker(statusEvery)
	defer ticker.Stop()
	for {
		select {
		case <-c.ctx.Done():
			return
		case <-ticker.C:
		}
		c.mu.Lock()
		unconfirmed := c.sent > c.upBase || c.received > c.confirmed
		c.mu.Unlock()
		if unconfirmed {
			if err := c.status(); err != nil {
				c.fail(fmt.Errorf("asking the tunnel how far it got: %w", err))
				return
			}
		}
	}
}

// Streams what is written up one POST after another, each starting where the server got to.
func (c *tunnelConn) upload() {
	failures := 0
	for first := true; c.ctx.Err() == nil; first = false {
		if !first {
			if err := c.status(); err != nil {
				c.fail(fmt.Errorf("asking the tunnel how far it got: %w", err))
				return
			}
		}
		c.mu.Lock()
		c.sent = c.upBase
		offset := c.sent
		c.stats.posts++
		c.mu.Unlock()

		reader, writer := io.Pipe()
		r, err := c.request(c.ctx, http.MethodPost, "stream", offset, reader)
		if err != nil {
			c.fail(err)
			return
		}
		r.ContentLength = -1
		var over atomic.Bool
		answered := make(chan error, 1)
		go func() {
			resp, err := c.client.Do(r)
			if err == nil {
				if resp.StatusCode != http.StatusOK {
					err = statusError(resp)
				}
				io.Copy(io.Discard, resp.Body)
				resp.Body.Close()
			}
			if err == nil {
				err = io.EOF // a POST ends only when this side ends it
			}
			reader.CloseWithError(err)
			over.Store(true)
			c.mu.Lock()
			c.changed.Broadcast()
			c.mu.Unlock()
			answered <- err
		}()
		c.pump(writer, &over)
		writer.Close()
		err = <-answered
		if errors.As(err, new(permanent)) {
			c.fail(fmt.Errorf("streaming to the tunnel: %w", err))
			return
		}
		if c.ctx.Err() != nil {
			return
		}
		if err != io.EOF {
			failures++
			if failures >= tunnelAttempts {
				c.fail(fmt.Errorf("streaming to the tunnel: %w", err))
				return
			}
			backOff(c.ctx, failures)
		} else {
			failures = 0
		}
	}
}

// Writes what is written into one POST's body as it comes, until upLife has passed or the POST is over.
func (c *tunnelConn) pump(body *io.PipeWriter, over *atomic.Bool) {
	deadline := time.Now().Add(upLife)
	timer := time.AfterFunc(upLife, func() {
		c.mu.Lock()
		c.changed.Broadcast()
		c.mu.Unlock()
	})
	defer timer.Stop()
	for {
		c.mu.Lock()
		for c.sent >= c.upBase+int64(len(c.up)) && c.err == nil && !over.Load() && time.Now().Before(deadline) {
			c.changed.Wait()
		}
		if c.err != nil || over.Load() || !time.Now().Before(deadline) {
			c.mu.Unlock()
			return
		}
		from := c.sent - c.upBase
		chunk := append([]byte(nil), c.up[from:min(int64(len(c.up)), from+256<<10)]...)
		c.mu.Unlock()
		if _, err := body.Write(chunk); err != nil {
			return
		}
		c.mu.Lock()
		c.sent += int64(len(chunk))
		c.stats.sent += int64(len(chunk))
		c.mu.Unlock()
	}
}

// Reads the server's stream, asking again from where it got to whenever a GET ends.
func (c *tunnelConn) receive() {
	failures := 0
	buffer := make([]byte, 256<<10)
	for c.ctx.Err() == nil {
		c.mu.Lock()
		offset := c.received
		c.stats.gets++
		c.mu.Unlock()
		r, err := c.request(c.ctx, http.MethodGet, "down", offset, nil)
		if err != nil {
			c.fail(err)
			return
		}
		resp, err := c.client.Do(r)
		if err == nil && resp.StatusCode != http.StatusOK {
			err = statusError(resp)
			resp.Body.Close()
			if errors.As(err, new(permanent)) {
				c.fail(err)
				return
			}
		}
		if err != nil {
			failures++
			if failures >= tunnelAttempts {
				c.fail(fmt.Errorf("receiving from the tunnel: %w", err))
				return
			}
			backOff(c.ctx, failures)
			continue
		}
		for {
			n, readErr := resp.Body.Read(buffer)
			if n > 0 {
				failures = 0
				c.mu.Lock()
				for len(c.down) >= downBuffer && c.err == nil {
					c.changed.Wait()
				}
				c.down = append(c.down, buffer[:n]...)
				c.received += int64(n)
				c.changed.Broadcast()
				c.mu.Unlock()
			}
			if readErr != nil {
				break
			}
		}
		resp.Body.Close()
	}
}

func (c *tunnelConn) Read(p []byte) (int, error) {
	c.mu.Lock()
	defer c.mu.Unlock()
	for len(c.down) == 0 && c.err == nil {
		c.changed.Wait()
	}
	if len(c.down) == 0 {
		return 0, c.err
	}
	n := copy(p, c.down)
	c.down = c.down[n:]
	if len(c.down) == 0 {
		c.down = nil
	}
	c.changed.Broadcast()
	return n, nil
}

func (c *tunnelConn) Write(p []byte) (int, error) {
	c.mu.Lock()
	defer c.mu.Unlock()
	for len(c.up) >= upBuffer && c.err == nil {
		c.changed.Wait()
	}
	if c.err != nil {
		return 0, c.err
	}
	c.up = append(c.up, p...)
	c.changed.Broadcast()
	return len(p), nil
}

func (c *tunnelConn) Close() error {
	c.mu.Lock()
	open := c.err == nil
	c.mu.Unlock()
	c.fail(net.ErrClosed)
	if open {
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
			defer cancel()
			c.ask(ctx, http.MethodPost, "close", 0)
		}()
	}
	return nil
}

type tunnelAddr struct{}

func (tunnelAddr) Network() string { return "be3-tunnel" }
func (tunnelAddr) String() string  { return "be3-tunnel" }

func (c *tunnelConn) LocalAddr() net.Addr                { return tunnelAddr{} }
func (c *tunnelConn) RemoteAddr() net.Addr               { return tunnelAddr{} }
func (c *tunnelConn) SetDeadline(t time.Time) error      { return nil }
func (c *tunnelConn) SetReadDeadline(t time.Time) error  { return nil }
func (c *tunnelConn) SetWriteDeadline(t time.Time) error { return nil }

// ---- The server ----

type tunnelSession struct {
	conn net.Conn

	writing sync.Mutex // holds the order of what is written to conn
	next    int64      // the stream offset written up to

	mu        sync.Mutex
	changed   *sync.Cond
	down      []byte // sent or to send, from offset base
	base      int64
	ended     bool // conn has nothing more to say
	gets      int  // the GET streaming now, by number
	seen      time.Time
	streaming bool
	// Since the last report: bytes sent down, and the most that waited behind the GET to be sent.
	sent, mostQueued int64
}

type tunnelServer struct {
	upstream string
	mu       sync.Mutex
	sessions map[string]*tunnelSession
}

func serveTunnel(listen, upstream string) {
	t := &tunnelServer{upstream: upstream, sessions: map[string]*tunnelSession{}}
	go t.reap()
	server := &http.Server{Addr: listen, Handler: t.handler()}
	fmt.Fprintf(os.Stderr, "re-relay tunnel-server: listening on %s for %s\n", listen, upstream)
	if err := server.ListenAndServe(); err != nil {
		fail("%v", err)
	}
}

func (t *tunnelServer) handler() http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("POST "+tunnelPath+"/open", t.open)
	mux.HandleFunc("POST "+tunnelPath+"/stream", t.stream)
	mux.HandleFunc("GET "+tunnelPath+"/status", t.status)
	mux.HandleFunc("GET "+tunnelPath+"/down", t.down)
	mux.HandleFunc("POST "+tunnelPath+"/close", t.close)
	mux.HandleFunc("GET "+tunnelPath+"/probe-down", probeDown)
	mux.HandleFunc("POST "+tunnelPath+"/probe-up", probeUp)
	mux.HandleFunc("POST "+tunnelPath+"/probe-stream", probeStream)
	return mux
}

func query(r *http.Request) (string, int64, int64) {
	offset, _ := strconv.ParseInt(r.URL.Query().Get("off"), 10, 64)
	ack, _ := strconv.ParseInt(r.URL.Query().Get("ack"), 10, 64)
	return r.URL.Query().Get("s"), offset, ack
}

func (t *tunnelServer) session(id string) *tunnelSession {
	t.mu.Lock()
	defer t.mu.Unlock()
	return t.sessions[id]
}

func (t *tunnelServer) open(w http.ResponseWriter, r *http.Request) {
	id, _, _ := query(r)
	if len(id) != 32 {
		http.Error(w, "no session", http.StatusBadRequest)
		return
	}
	if t.session(id) != nil {
		return
	}
	conn, err := net.DialTimeout("tcp", t.upstream, 10*time.Second)
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadGateway)
		return
	}
	s := &tunnelSession{conn: conn, seen: time.Now()}
	s.changed = sync.NewCond(&s.mu)
	t.mu.Lock()
	if t.sessions[id] != nil {
		t.mu.Unlock()
		conn.Close()
		return
	}
	t.sessions[id] = s
	t.mu.Unlock()
	go s.read()
}

// Takes what the upstream sends into down, until the client is too far behind.
func (s *tunnelSession) read() {
	buffer := make([]byte, 256<<10)
	for {
		n, err := s.conn.Read(buffer)
		s.mu.Lock()
		if n > 0 {
			for len(s.down) >= downBuffer && !s.ended {
				s.changed.Wait()
			}
			s.down = append(s.down, buffer[:n]...)
		}
		if err != nil {
			s.ended = true
		}
		s.changed.Broadcast()
		s.mu.Unlock()
		if err != nil {
			return
		}
	}
}

// Forgets what the client says it has received. Called with s.mu held.
func (s *tunnelSession) acknowledge(ack int64) {
	if ack > s.base && ack <= s.base+int64(len(s.down)) {
		s.down = append(s.down[:0:0], s.down[ack-s.base:]...)
		s.base = ack
		s.changed.Broadcast()
	}
	s.seen = time.Now()
}

func (s *tunnelSession) end() {
	s.conn.Close()
	s.mu.Lock()
	s.ended = true
	s.changed.Broadcast()
	s.mu.Unlock()
}

// Writes what of b, which starts at stream offset at, has not been written yet.
func (s *tunnelSession) write(at int64, b []byte) error {
	s.writing.Lock()
	defer s.writing.Unlock()
	if at > s.next {
		return fmt.Errorf("the stream skipped from %d to %d", s.next, at)
	}
	if skip := s.next - at; skip < int64(len(b)) {
		if _, err := s.conn.Write(b[skip:]); err != nil {
			s.end()
			return err
		}
		s.next = at + int64(len(b))
	}
	s.mu.Lock()
	s.seen = time.Now()
	s.mu.Unlock()
	return nil
}

func (s *tunnelSession) written() int64 {
	s.writing.Lock()
	defer s.writing.Unlock()
	return s.next
}

// One POST's body, written to the upstream as it arrives; answers with how far the stream has been written.
func (t *tunnelServer) stream(w http.ResponseWriter, r *http.Request) {
	id, offset, ack := query(r)
	s := t.session(id)
	if s == nil {
		http.Error(w, "no such session", http.StatusGone)
		return
	}
	s.mu.Lock()
	s.acknowledge(ack)
	s.mu.Unlock()
	buffer := make([]byte, 256<<10)
	for {
		n, err := r.Body.Read(buffer)
		if n > 0 {
			if err := s.write(offset, buffer[:n]); err != nil {
				http.Error(w, err.Error(), http.StatusConflict)
				return
			}
			offset += int64(n)
		}
		if err != nil {
			break
		}
	}
	fmt.Fprint(w, s.written())
}

// How far the stream has been written, after forgetting what the client says it has received.
func (t *tunnelServer) status(w http.ResponseWriter, r *http.Request) {
	id, _, ack := query(r)
	s := t.session(id)
	if s == nil {
		http.Error(w, "no such session", http.StatusGone)
		return
	}
	s.mu.Lock()
	s.acknowledge(ack)
	s.mu.Unlock()
	fmt.Fprint(w, s.written())
}

func (t *tunnelServer) down(w http.ResponseWriter, r *http.Request) {
	id, offset, _ := query(r)
	s := t.session(id)
	if s == nil {
		http.Error(w, "no such session", http.StatusGone)
		return
	}
	s.mu.Lock()
	s.acknowledge(offset)
	if offset < s.base || offset > s.base+int64(len(s.down)) {
		s.mu.Unlock()
		http.Error(w, "that part of the stream is gone", http.StatusGone)
		return
	}
	if s.ended && offset == s.base+int64(len(s.down)) {
		s.mu.Unlock()
		http.Error(w, "the session ended", http.StatusGone)
		return
	}
	s.gets++
	mine := s.gets
	s.streaming = true
	s.mu.Unlock()

	// What proxies stream as it comes rather than collect first.
	w.Header().Set("Content-Type", "text/event-stream")
	w.Header().Set("Cache-Control", "no-store, no-transform")
	w.Header().Set("X-Accel-Buffering", "no")
	w.WriteHeader(http.StatusOK)
	flusher := w.(http.Flusher)
	flusher.Flush()

	deadline := time.Now().Add(downLife)
	wake := func() { s.mu.Lock(); s.changed.Broadcast(); s.mu.Unlock() }
	timer := time.AfterFunc(downLife, wake)
	defer timer.Stop()
	stop := context.AfterFunc(r.Context(), wake)
	defer stop()

	position := offset
	for {
		s.mu.Lock()
		for position >= s.base+int64(len(s.down)) && !s.ended && s.gets == mine &&
			r.Context().Err() == nil && time.Now().Before(deadline) {
			s.changed.Wait()
		}
		if s.gets != mine || r.Context().Err() != nil || position < s.base ||
			position >= s.base+int64(len(s.down)) {
			if s.gets == mine {
				s.streaming = false
			}
			s.seen = time.Now()
			s.mu.Unlock()
			return
		}
		from := position - s.base
		s.mostQueued = max(s.mostQueued, int64(len(s.down))-from)
		chunk := append([]byte(nil), s.down[from:min(int64(len(s.down)), from+256<<10)]...)
		s.sent += int64(len(chunk))
		s.seen = time.Now()
		s.mu.Unlock()
		if _, err := w.Write(chunk); err != nil {
			return
		}
		flusher.Flush()
		position += int64(len(chunk))
	}
}

func (t *tunnelServer) close(w http.ResponseWriter, r *http.Request) {
	id, _, _ := query(r)
	t.mu.Lock()
	s := t.sessions[id]
	delete(t.sessions, id)
	t.mu.Unlock()
	if s != nil {
		s.end()
	}
}

// Ends sessions no client has asked anything of for ten minutes, and every 30 s logs what each busy one sent and the
// most that queued behind its GET: what an answer may wait behind on its way down.
func (t *tunnelServer) reap() {
	for range time.Tick(30 * time.Second) {
		t.mu.Lock()
		for id, s := range t.sessions {
			s.mu.Lock()
			idle := !s.streaming && time.Since(s.seen) > 10*time.Minute
			if s.sent > 0 {
				fmt.Fprintf(os.Stderr, "tunnel %s: sent %d bytes, at most %d queued behind the GET\n", id[:8], s.sent, s.mostQueued)
			}
			s.sent, s.mostQueued = 0, 0
			s.mu.Unlock()
			if idle {
				delete(t.sessions, id)
				go s.end()
			}
		}
		t.mu.Unlock()
	}
}

// ---- Probing the way through a proxy ----

// Streams a line holding the server's clock, in Unix nanoseconds, every 100 ms for 10 s, padded to the size asked for:
// what tunnel-probe times on its way down.
func probeDown(w http.ResponseWriter, r *http.Request) {
	size, _ := strconv.Atoi(r.URL.Query().Get("size"))
	w.Header().Set("Content-Type", "text/event-stream")
	w.Header().Set("Cache-Control", "no-store, no-transform")
	w.Header().Set("X-Accel-Buffering", "no")
	flusher := w.(http.Flusher)
	for range 100 {
		line := strconv.FormatInt(time.Now().UnixNano(), 10)
		line += strings.Repeat(" ", max(0, size-len(line)-1)) + "\n"
		if _, err := io.WriteString(w, line); err != nil {
			return
		}
		flusher.Flush()
		select {
		case <-r.Context().Done():
			return
		case <-time.After(100 * time.Millisecond):
		}
	}
}

// Answers with the server's clock when the request arrived, in Unix nanoseconds.
func probeUp(w http.ResponseWriter, r *http.Request) {
	arrived := time.Now().UnixNano()
	io.Copy(io.Discard, r.Body)
	fmt.Fprint(w, arrived)
}

// Reads lines each holding the client's clock in Unix nanoseconds as they arrive, and answers with how long each
// took to arrive by the server's clock: whether the way up passes a request's body along as it comes.
func probeStream(w http.ResponseWriter, r *http.Request) {
	lines := bufio.NewReader(r.Body)
	var arrivals []string
	for {
		line, err := lines.ReadString('\n')
		if err != nil {
			break
		}
		sent, _ := strconv.ParseInt(strings.TrimSpace(line), 10, 64)
		arrivals = append(arrivals, strconv.FormatInt(time.Now().UnixNano()-sent, 10))
	}
	fmt.Fprint(w, strings.Join(arrivals, "\n"))
}

// tunnel-probe: times each way between here and upstream's tunnel-server through the environment's proxy, one way
// against the server's clock (both clocks are taken to be kept by NTP, so to a few milliseconds) and by how lines sent
// 100 ms apart arrive. Authorization comes from BE3_NL_TOKEN, when set.
func probeTunnel(upstream string) {
	client := &http.Client{Transport: &http.Transport{
		Proxy:              http.ProxyFromEnvironment,
		DisableCompression: true,
		TLSNextProto:       map[string]func(string, *tls.Conn) http.RoundTripper{},
	}}
	base := "https://" + upstream + tunnelPath
	request := func(method, url string, body io.Reader) *http.Request {
		r, err := http.NewRequest(method, url, body)
		if err != nil {
			fail("%v", err)
		}
		if token := os.Getenv("BE3_NL_TOKEN"); token != "" {
			r.Header.Set("Authorization", "Bearer "+token)
		}
		return r
	}
	ms := func(d time.Duration) string { return fmt.Sprintf("%.0f ms", float64(d)/float64(time.Millisecond)) }
	summary := func(what string, values []time.Duration) {
		if len(values) == 0 {
			fmt.Printf("%s: nothing\n", what)
			return
		}
		sorted := append([]time.Duration(nil), values...)
		slices.Sort(sorted)
		fmt.Printf("%s: min %s, median %s, 90th %s, max %s (%d)\n", what, ms(sorted[0]), ms(sorted[len(sorted)/2]),
			ms(sorted[len(sorted)*9/10]), ms(sorted[len(sorted)-1]), len(sorted))
	}

	var rtts, ups []time.Duration
	for range 20 {
		sent := time.Now()
		resp, err := client.Do(request(http.MethodPost, base+"/probe-up", strings.NewReader("probe")))
		if err != nil {
			fail("%v", err)
		}
		body, _ := io.ReadAll(resp.Body)
		resp.Body.Close()
		if resp.StatusCode != http.StatusOK {
			fail("probe-up answered %s: %s", resp.Status, body)
		}
		arrived, _ := strconv.ParseInt(strings.TrimSpace(string(body)), 10, 64)
		rtts = append(rtts, time.Since(sent))
		ups = append(ups, time.Duration(arrived-sent.UnixNano()))
	}
	summary("POST round trip", rtts)
	summary("POST one way up (server clock minus ours)", ups)

	// One POST whose body is a line every 100 ms for 5 s: if the proxy passes it along as it comes, each line arrives
	// in about the one-way time down; if it collects the whole body first, the first line arrives 5 s late.
	reader, writer := io.Pipe()
	go func() {
		for range 50 {
			fmt.Fprintf(writer, "%d\n", time.Now().UnixNano())
			time.Sleep(100 * time.Millisecond)
		}
		writer.Close()
	}()
	streaming := request(http.MethodPost, base+"/probe-stream", reader)
	streaming.ContentLength = -1
	resp, err := client.Do(streaming)
	if err != nil {
		fail("%v", err)
	}
	body, _ := io.ReadAll(resp.Body)
	resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		fail("probe-stream answered %s: %s", resp.Status, body)
	}
	var streamed []time.Duration
	for _, field := range strings.Fields(string(body)) {
		nanos, _ := strconv.ParseInt(field, 10, 64)
		streamed = append(streamed, time.Duration(nanos))
	}
	summary("One streaming POST, a line every 100 ms: one way up for each line", streamed)
	if len(streamed) > 0 {
		fmt.Printf("  first line %s, last line %s\n", ms(streamed[0]), ms(streamed[len(streamed)-1]))
	}

	for _, size := range []int{32, 4096} {
		resp, err := client.Do(request(http.MethodGet, fmt.Sprintf("%s/probe-down?size=%d", base, size), nil))
		if err != nil {
			fail("%v", err)
		}
		if resp.StatusCode != http.StatusOK {
			body, _ := io.ReadAll(resp.Body)
			fail("probe-down answered %s: %s", resp.Status, body)
		}
		lines := bufio.NewReader(resp.Body)
		var downs, gaps []time.Duration
		bunched := 0
		var last time.Time
		for {
			line, err := lines.ReadString('\n')
			if err != nil {
				break
			}
			now := time.Now()
			sent, _ := strconv.ParseInt(strings.TrimSpace(line), 10, 64)
			downs = append(downs, time.Duration(now.UnixNano()-sent))
			if !last.IsZero() {
				gap := now.Sub(last)
				gaps = append(gaps, gap)
				if gap < 20*time.Millisecond {
					bunched++
				}
			}
			last = now
		}
		resp.Body.Close()
		fmt.Printf("GET lines of %d bytes, sent every 100 ms: %d arrived within 20 ms of the one before\n", size, bunched)
		summary(fmt.Sprintf("  one way down, %d bytes (our clock minus the server's)", size), downs)
		summary(fmt.Sprintf("  gap between arrivals, %d bytes", size), gaps)
	}
}
