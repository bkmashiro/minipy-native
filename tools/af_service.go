// AF benchmark-only streaming entry. External driver owns bounded admission.
package main

import (
	"bufio"
	"encoding/json"
	"os"
	"sync"
	"sync/atomic"
)

func afServe(run func(string, int) string, loads, records *atomic.Int64) {
	expected := map[string]map[int]string{}
	for _, c := range []string{"scalar", "weighted", "stateful"} {
		expected[c] = map[int]string{}
		for _, n := range []int{0, 100, 10000, 100000} {
			expected[c][n] = oracle(c, n)
		}
	}
	var mu sync.Mutex
	send := func(v any) { mu.Lock(); defer mu.Unlock(); emit(v) }
	base := cpu()
	send(map[string]any{"kind": "ready", "cpu_ns": base, "mono_ns": monotonic(), "pid": os.Getpid()})
	var wg sync.WaitGroup
	scan := bufio.NewScanner(os.Stdin)
	for scan.Scan() {
		var req struct {
			ID      int    `json:"id"`
			Case    string `json:"case"`
			Rows    int    `json:"rows"`
			Planned int64  `json:"planned_ns"`
		}
		if e := json.Unmarshal(scan.Bytes(), &req); e != nil {
			panic(e)
		}
		if req.ID < 0 {
			break
		}
		want, ok := expected[req.Case][req.Rows]
		if !ok {
			panic("unknown shape")
		}
		wg.Add(1)
		go func() {
			defer wg.Done()
			begin := monotonic()
			out := run(req.Case, req.Rows+17)
			end := monotonic()
			send(map[string]any{"kind": "result", "id": req.ID, "start_ns": begin, "end_ns": end, "stdout": out, "ok": out == want})
		}()
	}
	if e := scan.Err(); e != nil {
		panic(e)
	}
	wg.Wait()
	send(map[string]any{"kind": "final", "cpu_ns": cpu() - base, "mono_ns": monotonic(), "loads": loads.Load(), "records": records.Load()})
}
