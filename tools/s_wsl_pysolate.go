// S old-Pysolate public-API adapter. No upstream repository changes.
// argv: fresh|cow CASE cold|mixed-cold|hold ROWS REPS DELAY_US FIXTURES GUEST
// This is a synchronous tools/JSON-object representation, NOT common-body parity.
package main

import (
	"context"
	"encoding/json"
	"fmt"
	pysolate "github.com/bkmashiro/agent-python-runtime"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"syscall"
	"time"
    "unsafe"
)

func emit(v any) {
	b, e := json.Marshal(v)
	if e != nil {
		panic(e)
	}
	fmt.Println(string(b))
}
func cpu() int64 {
	var r syscall.Rusage
	if e := syscall.Getrusage(syscall.RUSAGE_SELF, &r); e != nil {
		panic(e)
	}
	return r.Utime.Sec*1e9 + r.Utime.Usec*1000 + r.Stime.Sec*1e9 + r.Stime.Usec*1000
}
func physical() map[string]int64 {
	b, e := os.ReadFile("/proc/self/smaps_rollup")
	if e != nil {
		panic(e)
	}
	out := map[string]int64{}
	for _, l := range strings.Split(string(b), "\n") {
		f := strings.Fields(l)
		if len(f) > 1 && (f[0] == "Rss:" || f[0] == "Pss:") {
			v, e := strconv.ParseInt(f[1], 10, 64)
			if e != nil {
				panic(e)
			}
			if f[0] == "Rss:" {
				out["rss_bytes"] = v * 1024
			} else {
				out["pss_bytes"] = v * 1024
			}
		}
	}
	return out
}
func oracle(c string, n int) string {
	if c == "scalar" {
		return "734\nTrue\n"
	}
	var total, prev int64
	for i := 0; i < n; i++ {
		p, q := int64(i*37%1000), int64(i%9-2)
		if q > 0 && p <= 700 && (c != "stateful" || p >= prev) {
			total += p * q
			if c == "stateful" {
				prev = p
			}
		}
	}
	return fmt.Sprintf("%d\n", total+17)
}
func source(dir, c string) string {
	b, e := os.ReadFile(filepath.Join(dir, "common_"+c+".py"))
	if e != nil {
		panic(e)
	}
	s := string(b)
	s = strings.ReplaceAll(s, "quotes = await tools.load_quotes(batch_id)", "quotes = [SimpleNamespace(**q) for q in tools.load_quotes(batch_id=batch_id)]")
	s = strings.ReplaceAll(s, "await tools.record_total(batch_id, total)", "tools.record_total(batch_id=batch_id, total=total)")
	return "from types import SimpleNamespace\nbatch_id = inputs['batch_id']\nprice_limit = inputs['price_limit']\n" + s
}
func monotonic() int64 {
    var ts syscall.Timespec
    _, _, err := syscall.Syscall(syscall.SYS_CLOCK_GETTIME, 1, uintptr(unsafe.Pointer(&ts)), 0)
    if err != 0 { panic(err) }
    return ts.Sec*1e9 + ts.Nsec
}
func main() {
    initBegin := monotonic()
	if len(os.Args) != 9 {
		panic("args")
	}
	arm, c, mode := os.Args[1], os.Args[2], os.Args[3]
	n, _ := strconv.Atoi(os.Args[4])
	reps, _ := strconv.Atoi(os.Args[5])
	delay, _ := strconv.Atoi(os.Args[6])
	dir, guest := os.Args[7], os.Args[8]
	var loads, records atomic.Int64
	var loadGate, recordGate chan struct{}
	if mode == "hold" {
		loadGate = make(chan struct{})
		recordGate = make(chan struct{})
	}
	manifest := pysolate.Manifest{
		"load": {PythonPath: "tools.load_quotes", Call: func(ctx context.Context, raw json.RawMessage) (any, error) {
			var a struct {
				Batch int `json:"batch_id"`
			}
			if e := json.Unmarshal(raw, &a); e != nil {
				return nil, e
			}
			if a.Batch < 0 {
				return nil, fmt.Errorf("bad batch")
			}
			rows := n
			if mode == "mixed-cold" {
				rows = a.Batch - 17
			}
			qs := make([]map[string]int64, rows)
			for i := range qs {
				qs[i] = map[string]int64{"unit_price_cents": int64(i * 37 % 1000), "quantity": int64(i%9 - 2)}
			}
			loads.Add(1)
			if loadGate != nil {
				select {
				case <-loadGate:
				case <-ctx.Done():
					return nil, ctx.Err()
				}
			}
			time.Sleep(time.Duration(delay) * time.Microsecond)
			return qs, nil
		}},
		"record": {PythonPath: "tools.record_total", Call: func(ctx context.Context, raw json.RawMessage) (any, error) {
			var a struct{ Batch, Total int64 }
			if e := json.Unmarshal(raw, &a); e != nil {
				return nil, e
			}
			var fields map[string]int64
			if e := json.Unmarshal(raw, &fields); e != nil {
				return nil, e
			}
			records.Add(1)
			if recordGate != nil {
				select {
				case <-recordGate:
				case <-ctx.Done():
					return nil, ctx.Err()
				}
			}
			time.Sleep(time.Duration(delay) * time.Microsecond)
			return fields["total"] + 17, nil
		}},
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Minute)
	defer cancel()
	t, cp := time.Now(), cpu()
	wasm, e := os.ReadFile(guest)
	if e != nil {
		panic(e)
	}
	var r *pysolate.Runner
	if arm == "cow" {
		r, e = pysolate.NewPreparedCOW(ctx, wasm, manifest, pysolate.COWOptions{DataImage: true})
	} else if arm == "fresh" {
		r, e = pysolate.New(ctx, wasm, manifest)
	} else {
		panic("arm")
	}
	if e != nil {
		panic(e)
	}
	defer r.Close(context.Background())
	engine := "pysolate-" + arm
	emit(map[string]any{"kind": "initialization", "engine": engine, "wall_ns": time.Since(t).Nanoseconds(), "cpu_ns": cpu() - cp, "physical": physical(), "includes": "Guest artifact read, Wasm compile, optional prepared COW image capture; imports and process startup excluded"})
    if mode == "first-request" {
        ready := monotonic()
        s := source(dir, c)
        prepared := monotonic()
        out, err := r.Run(ctx, s, map[string]any{"batch_id":17,"price_limit":700})
        executed := monotonic()
        if err != nil { panic(err) }
        if out.Stdout != oracle(c,n) { panic("stdout") }
        emit(map[string]any{"kind":"first-request","engine":engine,"case":c,"rows":n,
            "init_begin_ns":initBegin,"ready_ns":ready,"prepared_ns":prepared,"executed_ns":executed,
            "stdout":out.Stdout,"completed":true,"prepare_boundary":"host source adaptation only",
            "execute_boundary":"Runner.Run includes Guest Python compilation, instance/data image use, generated JSON input, tools and output"})
        return
    }
	run := func(c string, n int, batch int) {
		out, e := r.Run(ctx, source(dir, c), map[string]any{"batch_id": batch, "price_limit": 700})
		if e != nil {
			panic(e)
		}
		if out.Stdout != oracle(c, n) {
			panic(fmt.Sprintf("stdout %q want %q", out.Stdout, oracle(c, n)))
		}
	}
	if mode == "cold" {
		for i := 0; i < reps; i++ {
			l, k := loads.Load(), records.Load()
			t, cp := time.Now(), cpu()
			run(c, n, 17)
			wall, cost := time.Since(t).Nanoseconds(), cpu()-cp
			want := int64(1)
			if c == "scalar" {
				want = 0
			}
			if loads.Load()-l != want || records.Load()-k != want {
				panic("tool count")
			}
			emit(map[string]any{"kind": "speed", "engine": engine, "case": c, "rows": n, "mode": "cold", "sample": i, "wall_ns": wall, "cpu_ns": cost, "completed": true, "corpus": "old-pysolate-adapted"})
		}
		return
	}
	if mode == "hold" {
		snap := func(p string) {
			emit(map[string]any{"kind": "hold", "engine": engine, "case": c, "rows": n, "count": reps, "code": "runner-shared", "phase": p, "physical": physical(), "requested_traced_bytes": nil, "corpus": "old-pysolate-adapted"})
		}
		snap("startup")
		var wg sync.WaitGroup
		for i := 0; i < reps; i++ {
			wg.Add(1)
			go func() { defer wg.Done(); run(c, n, 17) }()
		}
		for loads.Load() != int64(reps) {
			time.Sleep(time.Millisecond)
		}
		snap("load_wait")
		close(loadGate)
		for records.Load() != int64(reps) {
			time.Sleep(time.Millisecond)
		}
		snap("record_wait")
		close(recordGate)
		wg.Wait()
		snap("reclaimed")
		return
	}
	if mode != "mixed-cold" {
		panic("mode")
	}
	shapes := []struct {
		c string
		n int
	}{{"scalar", 0}, {"weighted", 100}, {"stateful", 100}, {"weighted", 10000}, {"stateful", 100000}}
	weights := []int{24, 8, 4, 2, 2}
	if c == "scan" {
		weights = []int{8, 16, 8, 4, 4}
	}
	schedule := []int{}
	for i, w := range weights {
		for j := 0; j < w; j++ {
			schedule = append(schedule, i)
		}
	}
	for sample := 0; sample < reps; sample++ {
		l, k := loads.Load(), records.Load()
		t, cp := time.Now(), cpu()
		rows := make([]map[string]any, 40)
		var wg sync.WaitGroup
		for i := 0; i < 40; i++ {
			wg.Add(1)
			go func(i int) {
				defer wg.Done()
				s := shapes[schedule[i*17%40]]
				due := t.Add(time.Duration(i) * 500 * time.Microsecond)
				time.Sleep(time.Until(due))
				run(s.c, s.n, 17+s.n)
				rows[i] = map[string]any{"kind": "mixed", "engine": engine, "mix": c, "case": s.c, "rows": s.n, "mode": "mixed-cold", "sample": sample, "arrival": i, "delay_us": delay, "inflight": 40, "latency_ns": time.Since(due).Nanoseconds(), "completed": true, "corpus": "old-pysolate-adapted"}
			}(i)
		}
		wg.Wait()
		wall, cost := time.Since(t).Nanoseconds(), cpu()-cp
		want := int64(40 - weights[0])
		if loads.Load()-l != want || records.Load()-k != want {
			panic("mixed tool count")
		}
		for _, row := range rows {
			row["batch_wall_ns"] = wall
			row["batch_cpu_ns"] = cost
			row["overloaded"] = wall > 100000000
			emit(row)
		}
		time.Sleep(time.Until(t.Add(100 * time.Millisecond)))
	}
}
