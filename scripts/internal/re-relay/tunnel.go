package main

// A tunnel for a machine that cannot reach the build server's WireGuard network. Boat's hosted-port proxy gives the
// scheduler an HTTPS address; it carries WebSockets but blocks gRPC, so each TCP connection buck2 opens
// to the listening address here is carried to the scheduler inside one WebSocket, where
// scripts/internal/nativelink/scheduler/nl-tunnel.py turns it back into a TCP connection to NativeLink. buck2's HTTP/2
// goes through untouched, so unlike the relay above this knows nothing about gRPC.
//
//	re-relay serve-tunnel  -listen 127.0.0.1:18981 -url wss://HOST/ -version V
//	re-relay ensure-tunnel -listen 127.0.0.1:18981 -url wss://HOST/ -version V -state FILE -log FILE
//
// The bridge's token travels in the environment, not on a command line: BE3_TUNNEL_TOKEN is sent as
// "Authorization: Bearer TOKEN" with every upgrade. Without it no header is sent, for an HTTPS proxy that adds it (a
// Claude Code cloud session's network secret). The upgrade is made through net/http, so an HTTPS proxy in the
// environment carries it with CONNECT.

import (
	"bufio"
	"crypto/rand"
	"crypto/sha1"
	"crypto/sha256"
	"crypto/tls"
	"encoding/base64"
	"encoding/binary"
	"encoding/hex"
	"errors"
	"flag"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"strconv"
	"strings"
	"sync"
	"time"
)

const (
	wsGUID      = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
	wsPingEvery = 20 * time.Second
	wsChunk     = 32 * 1024
	wsDialTries = 3
	opContinue  = 0x0
	opText      = 0x1
	opBinary    = 0x2
	opClose     = 0x8
	opPing      = 0x9
	opPong      = 0xA
	tunnelToken = "BE3_TUNNEL_TOKEN"
)

// tunnelCommand runs the tunnel's subcommands and reports whether name was one of them.
func tunnelCommand(name string, args []string) bool {
	switch name {
	case "serve-tunnel":
		tunnelServe(args)
	case "ensure-tunnel":
		tunnelEnsure(args)
	default:
		return false
	}
	return true
}

type tunnelFlags struct {
	listen, url, version, state, logPath string
}

func parseTunnelFlags(name string, args []string) tunnelFlags {
	var t tunnelFlags
	flags := flag.NewFlagSet(name, flag.ExitOnError)
	flags.StringVar(&t.listen, "listen", "127.0.0.1:18981", "the address buck2 is pointed at")
	flags.StringVar(&t.url, "url", "", "the scheduler's wss:// address, without its token")
	flags.StringVar(&t.version, "version", "", "what ensure expects a running tunnel to be")
	flags.StringVar(&t.state, "state", "", "where ensure records the running tunnel")
	flags.StringVar(&t.logPath, "log", "", "where a tunnel ensure starts writes its errors")
	flags.Parse(args)
	if t.url == "" {
		fail("-url is required")
	}
	return t
}

// tunnelEnsure returns once a tunnel of this version listens, starting one in the background if there is none and
// replacing one of another version.
func tunnelEnsure(args []string) {
	t := parseTunnelFlags("ensure-tunnel", args)
	// A changed token replaces the tunnel too; the state file records only its hash.
	sum := sha256.Sum256([]byte(os.Getenv(tunnelToken)))
	want := t.version + " " + t.url + " " + hex.EncodeToString(sum[:8])
	if t.state != "" {
		if text, err := os.ReadFile(t.state); err == nil {
			version, pid, _ := strings.Cut(strings.TrimSpace(string(text)), "\n")
			if version == want && listening(t.listen) {
				return
			}
			if number, err := strconv.Atoi(pid); err == nil {
				if process, err := os.FindProcess(number); err == nil {
					process.Kill()
					time.Sleep(200 * time.Millisecond)
				}
			}
		}
	}
	self, err := os.Executable()
	if err != nil {
		fail("%v", err)
	}
	command := exec.Command(self, "serve-tunnel", "-listen", t.listen, "-url", t.url)
	detach(command)
	if t.logPath != "" {
		log, err := os.OpenFile(t.logPath, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
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
	line = strings.TrimSpace(line)
	if line != "ready" {
		fail("the tunnel did not start: %s", strings.TrimPrefix(line, "error: "))
	}
	if t.state != "" {
		os.WriteFile(t.state, []byte(want+"\n"+strconv.Itoa(command.Process.Pid)+"\n"), 0o600)
	}
}

func listening(address string) bool {
	connection, err := net.DialTimeout("tcp", address, time.Second)
	if err != nil {
		return false
	}
	connection.Close()
	return true
}

func tunnelServe(args []string) {
	t := parseTunnelFlags("serve-tunnel", args)
	token := os.Getenv(tunnelToken)
	// One upgrade now, so that a wrong address or token is an error here and not a build that hangs.
	probe, err := wsDial(t.url, token)
	if err != nil {
		fmt.Println("error: " + err.Error())
		os.Stdout.Close()
		os.Exit(1)
	}
	probe.close()
	listener, err := net.Listen("tcp", t.listen)
	if err != nil {
		fmt.Println("error: " + err.Error())
		os.Stdout.Close()
		os.Exit(1)
	}
	fmt.Println("ready")
	os.Stdout.Close()
	for {
		connection, err := listener.Accept()
		if err != nil {
			fail("%v", err)
		}
		go tunnelConnection(connection, t.url, token)
	}
}

func tunnelConnection(local net.Conn, url, token string) {
	defer local.Close()
	var ws *wsConn
	var err error
	for try := 0; try < wsDialTries; try++ {
		if ws, err = wsDial(url, token); err == nil {
			break
		}
		time.Sleep(time.Duration(try+1) * 300 * time.Millisecond)
	}
	if err != nil {
		fmt.Fprintf(os.Stderr, "tunnel: %v\n", err)
		return
	}
	ws.pump(local)
}

var tunnelTransport = &http.Transport{
	Proxy:               http.ProxyFromEnvironment,
	ForceAttemptHTTP2:   false,
	TLSNextProto:        map[string]func(string, *tls.Conn) http.RoundTripper{}, // an upgrade is HTTP/1.1
	DisableCompression:  true,
	DialContext:         (&net.Dialer{Timeout: 20 * time.Second}).DialContext,
	TLSHandshakeTimeout: 20 * time.Second,
}

// wsConn is a client WebSocket: it writes masked frames and reads the server's.
type wsConn struct {
	stream io.ReadWriteCloser
	reader *bufio.Reader
	write  sync.Mutex
}

func wsDial(url, token string) (*wsConn, error) {
	address := strings.Replace(strings.Replace(url, "wss://", "https://", 1), "ws://", "http://", 1)
	request, err := http.NewRequest("GET", address, nil)
	if err != nil {
		return nil, err
	}
	nonce := make([]byte, 16)
	rand.Read(nonce)
	key := base64.StdEncoding.EncodeToString(nonce)
	request.Header.Set("Connection", "Upgrade")
	request.Header.Set("Upgrade", "websocket")
	request.Header.Set("Sec-WebSocket-Version", "13")
	request.Header.Set("Sec-WebSocket-Key", key)
	if token != "" {
		request.Header.Set("Authorization", "Bearer "+token)
	}
	response, err := tunnelTransport.RoundTrip(request)
	if err != nil {
		return nil, fmt.Errorf("cannot reach %s: %w", url, err)
	}
	if response.StatusCode != http.StatusSwitchingProtocols {
		snippet, _ := io.ReadAll(io.LimitReader(response.Body, 200))
		response.Body.Close()
		hint := ""
		if response.StatusCode == http.StatusForbidden || response.StatusCode == http.StatusUnauthorized {
			hint = " (the token is missing or wrong)"
		}
		return nil, fmt.Errorf("%s answered %d%s: %s", url, response.StatusCode, hint, strings.TrimSpace(string(snippet)))
	}
	stream, ok := response.Body.(io.ReadWriteCloser)
	if !ok {
		return nil, errors.New("the proxy did not hand over the connection after the upgrade")
	}
	sum := sha1.Sum([]byte(key + wsGUID))
	if response.Header.Get("Sec-WebSocket-Accept") != base64.StdEncoding.EncodeToString(sum[:]) {
		stream.Close()
		return nil, errors.New("the server's WebSocket answer was wrong")
	}
	return &wsConn{stream: stream, reader: bufio.NewReaderSize(stream, 64*1024)}, nil
}

func (c *wsConn) writeFrame(opcode byte, payload []byte) error {
	var head [14]byte
	head[0] = 0x80 | opcode
	n := len(payload)
	size := 2
	switch {
	case n < 126:
		head[1] = 0x80 | byte(n)
	case n < 65536:
		head[1] = 0x80 | 126
		binary.BigEndian.PutUint16(head[2:], uint16(n))
		size = 4
	default:
		head[1] = 0x80 | 127
		binary.BigEndian.PutUint64(head[2:], uint64(n))
		size = 10
	}
	mask := head[size : size+4]
	rand.Read(mask)
	masked := make([]byte, n)
	for i := range payload {
		masked[i] = payload[i] ^ mask[i%4]
	}
	c.write.Lock()
	defer c.write.Unlock()
	if _, err := c.stream.Write(head[:size+4]); err != nil {
		return err
	}
	_, err := c.stream.Write(masked)
	return err
}

func (c *wsConn) readFrame() (opcode byte, payload []byte, err error) {
	var head [2]byte
	if _, err = io.ReadFull(c.reader, head[:]); err != nil {
		return
	}
	opcode = head[0] & 0x0F
	length := uint64(head[1] & 0x7F)
	switch length {
	case 126:
		var extra [2]byte
		if _, err = io.ReadFull(c.reader, extra[:]); err != nil {
			return
		}
		length = uint64(binary.BigEndian.Uint16(extra[:]))
	case 127:
		var extra [8]byte
		if _, err = io.ReadFull(c.reader, extra[:]); err != nil {
			return
		}
		length = binary.BigEndian.Uint64(extra[:])
	}
	var mask [4]byte
	if head[1]&0x80 != 0 {
		if _, err = io.ReadFull(c.reader, mask[:]); err != nil {
			return
		}
	}
	payload = make([]byte, length)
	if _, err = io.ReadFull(c.reader, payload); err != nil {
		return
	}
	if head[1]&0x80 != 0 {
		for i := range payload {
			payload[i] ^= mask[i%4]
		}
	}
	return
}

func (c *wsConn) close() {
	c.writeFrame(opClose, []byte{0x03, 0xE8})
	c.stream.Close()
}

// pump copies bytes between the local connection and the WebSocket until either side ends.
func (c *wsConn) pump(local net.Conn) {
	done := make(chan struct{})
	var once sync.Once
	finish := func() {
		once.Do(func() {
			close(done)
			c.close()
			local.Close()
		})
	}
	defer finish()
	go func() { // keeps an idle connection from being cut by a proxy
		ticker := time.NewTicker(wsPingEvery)
		defer ticker.Stop()
		for {
			select {
			case <-ticker.C:
				if c.writeFrame(opPing, nil) != nil {
					finish()
					return
				}
			case <-done:
				return
			}
		}
	}()
	go func() { // local to server
		buffer := make([]byte, wsChunk)
		for {
			n, err := local.Read(buffer)
			if n > 0 && c.writeFrame(opBinary, buffer[:n]) != nil {
				finish()
				return
			}
			if err != nil {
				finish()
				return
			}
		}
	}()
	for { // server to local
		opcode, payload, err := c.readFrame()
		if err != nil {
			return
		}
		switch opcode {
		case opContinue, opText, opBinary:
			if _, err := local.Write(payload); err != nil {
				return
			}
		case opPing:
			if c.writeFrame(opPong, payload) != nil {
				return
			}
		case opClose:
			return
		}
	}
}
