// The tunnel: one byte stream between the relay and the build server carried by a few long HTTP/1.1 requests, so
// that HTTP/2 can run inside it and multiplex every call on it. A Claude Code cloud session's HTTPS proxy speaks only
// HTTP/1.1 and handles about 240 requests a second however many connections carry them; a cache-warm
// ./scripts/verify makes about 24,000 calls, so each call as its own request spends about 100 s in that limit. Through
// the tunnel the proxy sees one long GET carrying everything the server sends, and POSTs each carrying whatever the
// relay wrote while the ones before it were on their way: a few dozen requests a second at most.
//
// The client side is a net.Conn (tunnelConn) that an HTTP/2 transport dials. Its bytes to the server go in POSTs to
// /__be3-tunnel/up, numbered by their offset in the stream so the server writes them in order whatever order they
// arrive in, and retried as they were when one fails. The server's bytes come down a GET to /__be3-tunnel/down that
// streams from the offset the client asks for; when the proxy cuts it, or the server ends it after downLife to stay
// clear of the proxy's five-minute cut, the client asks again from where it got to. Every request says how much the
// client has received, and the server keeps what it sent until then.
//
// The server side (tunnel-server, behind Caddy, which checks the token) joins each session to one TCP connection to
// NativeLink's cleartext HTTP/2 port.
package main

import (
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
	"strconv"
	"sync"
	"sync/atomic"
	"time"
)

const tunnelPath = "/__be3-tunnel"

const (
	// The most the relay has written and not yet sent before Write waits, and the most one POST carries.
	upBuffer = 16 << 20
	upChunk  = 4 << 20
	// POSTs on their way at once. Each costs the proxy one request; while they are all out, what is written next
	// waits for one to come back and goes with everything else written meanwhile.
	upInFlight = 16
	// What HTTP/2 writes for itself (WINDOW_UPDATE as a download is read, PING, SETTINGS) waits up to lazyDelay for
	// something more to go with it, and takes at most lazyInFlight POSTs: a download sends a stream of them, which
	// would otherwise keep every POST busy and hold each new call back for a POST's round trip.
	lazyInFlight = 2
	lazyDelay    = 50 * time.Millisecond
	// The most the client holds unread, and the most the server keeps sent and not yet received, before each stops
	// reading what feeds it.
	downBuffer = 64 << 20
	// Requests that fail in a row before the tunnel gives up.
	tunnelAttempts = 8
)

// How long the server streams one GET before ending it, under the proxy's five-minute cut. A variable for the tests.
var downLife = 4 * time.Minute

// ---- The client ----

// An HTTP/2 transport whose every connection is a tunnel to upstream's tunnel-server, through the environment's
// proxy. authorization is the Authorization header the tunnel's requests carry, empty for a proxy that adds it.
func tunnelTransport(upstream string, authorization *atomic.Pointer[string]) http.RoundTripper {
	outer := &http.Transport{
		Proxy:               http.ProxyFromEnvironment,
		MaxConnsPerHost:     4 * (upInFlight + 2),
		MaxIdleConnsPerHost: 4 * (upInFlight + 2),
		IdleConnTimeout:     90 * time.Second,
		DisableCompression:  true,
		TLSNextProto:        map[string]func(string, *tls.Conn) http.RoundTripper{},
	}
	return tunnelTransportTo("https://"+upstream+tunnelPath, outer, authorization)
}

func tunnelTransportTo(base string, outer http.RoundTripper, authorization *atomic.Pointer[string]) http.RoundTripper {
	inner := &http.Transport{
		// One tunnel carries every call: HTTP/2 multiplexes them on it, and each more would cost the proxy a GET
		// held open and POSTs of its own.
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

	mu       sync.Mutex
	changed  *sync.Cond
	up       []byte // written and not yet handed to a POST
	upOffset int64  // the stream offset of up[0]
	inFlight int
	down     []byte // received and not yet read
	received int64  // bytes received in all
	err      error  // why the tunnel ended

	// Whether up holds more than HTTP/2's housekeeping, and since when it held anything, or anything more.
	urgent, timing         bool
	lazySince, urgentSince time.Time
	preface, frameLeft     int // HTTP/2's frames in what is written: the preface and frame body still to come
	header                 []byte
	stats                  tunnelStats
}

// What a tunnel did since it last said, for re-relay's log.
type tunnelStats struct {
	posts, lazyPosts, gets int
	postTime, maxPost      time.Duration
	waitTime, maxWait      time.Duration
	sent                   int64
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
		preface:       len("PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"),
	}
	c.changed = sync.NewCond(&c.mu)
	c.ctx, c.cancel = context.WithCancel(context.Background())
	if err := c.post(ctx, "open", 0, nil); err != nil {
		c.cancel()
		return nil, fmt.Errorf("opening the tunnel: %w", err)
	}
	go c.send()
	go c.receive()
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
		if s.posts == 0 {
			continue
		}
		fmt.Fprintf(os.Stderr, "re-relay: %s tunnel %s: %d POSTs (%d only HTTP/2's own), each %v on average and %v at most; "+
			"calls waited %v on average and %v at most for one; %d GETs; %d bytes sent, %d received in all\n",
			time.Now().Format(time.RFC3339), c.session[:8], s.posts, s.lazyPosts,
			(s.postTime / time.Duration(s.posts)).Round(time.Millisecond), s.maxPost.Round(time.Millisecond),
			(s.waitTime / time.Duration(max(1, s.posts-s.lazyPosts))).Round(time.Millisecond), s.maxWait.Round(time.Millisecond),
			s.gets, s.sent, received)
	}
}

// Notes whether p, written after what came before, holds more than HTTP/2's housekeeping frames. Called with c.mu
// held.
func (c *tunnelConn) scan(p []byte) {
	for len(p) > 0 {
		switch {
		case c.preface > 0:
			n := min(c.preface, len(p))
			c.preface -= n
			p = p[n:]
			c.urgent = true
		case c.frameLeft > 0:
			n := min(c.frameLeft, len(p))
			c.frameLeft -= n
			p = p[n:]
		default:
			n := min(9-len(c.header), len(p))
			c.header = append(c.header, p[:n]...)
			p = p[n:]
			if len(c.header) == 9 {
				c.frameLeft = int(c.header[0])<<16 | int(c.header[1])<<8 | int(c.header[2])
				switch c.header[3] {
				case 0x4, 0x6, 0x8: // SETTINGS, PING, WINDOW_UPDATE
				default:
					c.urgent = true
				}
				c.header = c.header[:0]
			}
		}
	}
}

// A request to the tunnel-server that should be made again when it fails, and why it is not worth making again.
type permanent struct{ error }

func (c *tunnelConn) request(ctx context.Context, method, what string, offset int64, body []byte) (*http.Request, error) {
	c.mu.Lock()
	ack := c.received
	c.mu.Unlock()
	url := fmt.Sprintf("%s/%s?s=%s&off=%d&ack=%d", c.base, what, c.session, offset, ack)
	var reader io.Reader
	if body != nil {
		reader = bytes.NewReader(body)
	}
	r, err := http.NewRequestWithContext(ctx, method, url, reader)
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
	case http.StatusGone, http.StatusNotFound, http.StatusUnauthorized, http.StatusForbidden:
		return permanent{err}
	}
	return err
}

// One POST, made again until it is answered or the tunnel ends.
func (c *tunnelConn) post(ctx context.Context, what string, offset int64, body []byte) error {
	var last error
	for attempt := 1; attempt <= tunnelAttempts; attempt++ {
		r, err := c.request(ctx, http.MethodPost, what, offset, body)
		if err != nil {
			return err
		}
		resp, err := c.client.Do(r)
		if err == nil {
			if resp.StatusCode == http.StatusOK {
				io.Copy(io.Discard, resp.Body)
				resp.Body.Close()
				return nil
			}
			err = statusError(resp)
			resp.Body.Close()
			if errors.As(err, new(permanent)) {
				return err
			}
		}
		last = err
		if ctx.Err() != nil {
			return ctx.Err()
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-time.After(time.Duration(100<<min(attempt, 5)) * time.Millisecond):
		}
	}
	return last
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

// Whether what was written goes in a POST now. Called with c.mu held; when it is only HTTP/2's housekeeping, too
// new to send, it wakes send when it will be old enough.
func (c *tunnelConn) ready() bool {
	if len(c.up) == 0 {
		return false
	}
	if c.urgent {
		return c.inFlight < upInFlight
	}
	if c.inFlight >= lazyInFlight {
		return false
	}
	wait := lazyDelay - time.Since(c.lazySince)
	if wait <= 0 {
		return true
	}
	if !c.timing {
		c.timing = true
		time.AfterFunc(wait, func() {
			c.mu.Lock()
			c.timing = false
			c.changed.Broadcast()
			c.mu.Unlock()
		})
	}
	return false
}

// Hands what was written to POSTs, upInFlight at a time.
func (c *tunnelConn) send() {
	for {
		c.mu.Lock()
		for !c.ready() && c.err == nil {
			c.changed.Wait()
		}
		if c.err != nil {
			c.mu.Unlock()
			return
		}
		n := min(len(c.up), upChunk)
		chunk := append([]byte(nil), c.up[:n]...)
		c.up = append(c.up[:0:0], c.up[n:]...)
		offset := c.upOffset
		c.upOffset += int64(n)
		c.inFlight++
		c.stats.posts++
		c.stats.sent += int64(n)
		if c.urgent {
			wait := time.Since(c.urgentSince)
			c.stats.waitTime += wait
			c.stats.maxWait = max(c.stats.maxWait, wait)
		} else {
			c.stats.lazyPosts++
		}
		if len(c.up) == 0 {
			c.urgent = false
		} else {
			c.lazySince, c.urgentSince = time.Now(), time.Now()
		}
		c.changed.Broadcast()
		c.mu.Unlock()
		go func() {
			start := time.Now()
			err := c.post(c.ctx, "up", offset, chunk)
			took := time.Since(start)
			c.mu.Lock()
			c.inFlight--
			c.stats.postTime += took
			c.stats.maxPost = max(c.stats.maxPost, took)
			c.changed.Broadcast()
			c.mu.Unlock()
			if err != nil {
				c.fail(fmt.Errorf("sending to the tunnel: %w", err))
			}
		}()
	}
}

// Reads the server's stream, asking again from where it got to whenever a GET ends.
func (c *tunnelConn) receive() {
	failures := 0
	buffer := make([]byte, 256<<10)
	for c.ctx.Err() == nil {
		c.mu.Lock()
		offset := c.received
		c.mu.Unlock()
		r, err := c.request(c.ctx, http.MethodGet, "down", offset, nil)
		if err != nil {
			c.fail(err)
			return
		}
		c.mu.Lock()
		c.stats.gets++
		c.mu.Unlock()
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
			select {
			case <-c.ctx.Done():
			case <-time.After(time.Duration(100<<min(failures, 5)) * time.Millisecond):
			}
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
	if len(c.up) == 0 {
		c.lazySince = time.Now()
	}
	wasUrgent := c.urgent
	c.scan(p)
	if c.urgent && !wasUrgent {
		c.urgentSince = time.Now()
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
			c.post(ctx, "close", 0, nil)
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
	early   map[int64][]byte

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
	mux.HandleFunc("POST "+tunnelPath+"/up", t.up)
	mux.HandleFunc("GET "+tunnelPath+"/down", t.down)
	mux.HandleFunc("POST "+tunnelPath+"/close", t.close)
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
	s := &tunnelSession{conn: conn, early: map[int64][]byte{}, seen: time.Now()}
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

func (t *tunnelServer) up(w http.ResponseWriter, r *http.Request) {
	id, offset, ack := query(r)
	s := t.session(id)
	if s == nil {
		http.Error(w, "no such session", http.StatusGone)
		return
	}
	body, err := io.ReadAll(io.LimitReader(r.Body, upChunk+1))
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	s.mu.Lock()
	s.acknowledge(ack)
	ended := s.ended
	s.mu.Unlock()
	if ended {
		http.Error(w, "the session ended", http.StatusGone)
		return
	}

	s.writing.Lock()
	defer s.writing.Unlock()
	if offset > s.next {
		if len(s.early) >= 4*upInFlight {
			http.Error(w, "too far ahead", http.StatusServiceUnavailable)
			return
		}
		s.early[offset] = body
		return
	}
	for {
		if skip := s.next - offset; skip < int64(len(body)) {
			if _, err := s.conn.Write(body[skip:]); err != nil {
				s.end()
				http.Error(w, err.Error(), http.StatusGone)
				return
			}
			s.next = offset + int64(len(body))
		}
		found := false
		for o, b := range s.early {
			if o <= s.next {
				delete(s.early, o)
				offset, body, found = o, b, true
				break
			}
		}
		if !found {
			return
		}
	}
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
