package main

import (
	"fmt"
	"os"
	"path"
	"slices"
	"sync"
	"time"
)

// How long buck2's calls took through the relay, by method, written to re-relay's log every 30 s that any were made:
// which calls a build makes one after another, and which of them a way to the server makes slow.
var timings = &callTimings{calls: map[string][]time.Duration{}}

type callTimings struct {
	mu    sync.Mutex
	calls map[string][]time.Duration
}

func (t *callTimings) add(method string, took time.Duration) {
	t.mu.Lock()
	defer t.mu.Unlock()
	if len(t.calls[method]) < 1_000_000 {
		t.calls[method] = append(t.calls[method], took)
	}
}

func (t *callTimings) report() {
	for range time.Tick(30 * time.Second) {
		t.mu.Lock()
		calls := t.calls
		t.calls = map[string][]time.Duration{}
		t.mu.Unlock()
		methods := make([]string, 0, len(calls))
		for method := range calls {
			methods = append(methods, method)
		}
		slices.Sort(methods)
		for _, method := range methods {
			took := calls[method]
			slices.Sort(took)
			at := func(fraction float64) time.Duration {
				return took[min(len(took)-1, int(fraction*float64(len(took))))].Round(time.Millisecond)
			}
			fmt.Fprintf(os.Stderr, "re-relay: %s calls %s: %d, median %v, 90th %v, max %v\n",
				time.Now().Format(time.RFC3339), path.Base(method), len(took), at(0.5), at(0.9), took[len(took)-1].Round(time.Millisecond))
		}
	}
}
