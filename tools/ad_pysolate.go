// AD corrected adapter and matched CPython JSON-object lifecycles.
// Original S/W adapter and raw are preserved; production runtime is untouched.
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

var native *nativeRunner

func cpu() int64 {
	var r syscall.Rusage
	if e := syscall.Getrusage(syscall.RUSAGE_SELF, &r); e != nil {
		panic(e)
	}
	cost := r.Utime.Sec*1e9 + r.Utime.Usec*1000 + r.Stime.Sec*1e9 + r.Stime.Usec*1000
	if native != nil {
		cost += native.cpu()
	}
	return cost
}
func physical() map[string]any {
	return treePhysical()
}
func pidPhysical(pid int) map[string]int64 {
	b, e := os.ReadFile(fmt.Sprintf("/proc/%d/smaps_rollup", pid))
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
	if err != 0 {
		panic(err)
	}
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
	cp := cpu()
	t := time.Now()
	var execute func(string, map[string]any) (string, error)
	engine := "pysolate-" + arm
	if arm == "fork" || arm == "resident" {
		native = startNative(manifest, arm, mode == "hold")
		defer native.close()
		engine = "cpython-" + arm + "-json"
		execute = native.run
	} else {
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
		execute = func(s string, in map[string]any) (string, error) {
			out, e := r.Run(ctx, s, in)
			return out.Stdout, e
		}
	}
	emit(map[string]any{"kind": "initialization", "engine": engine, "wall_ns": time.Since(t).Nanoseconds(), "cpu_ns": cpu() - cp, "physical": physical(), "includes": "runtime initialization; source preload/adaptation follows separately"})
	ready := monotonic()
	sources := map[string]string{}
	for _, c := range []string{"scalar", "weighted", "stateful"} {
		sources[c] = source(dir, c)
	}
	if native != nil {
		native.configure(sources)
	}
	prepared := monotonic()
	run := func(c string, batch int) string {
		out, e := execute(sources[c], map[string]any{"batch_id": batch, "price_limit": 700})
		if e != nil {
			panic(e)
		}
		return out
	}
	check := func(c string, n int, out string) {
		if want := oracle(c, n); out != want {
			panic(fmt.Sprintf("stdout %q want %q", out, want))
		}
	}
	if mode == "first-request" {
		out := run(c, 17)
		executed := monotonic()
		check(c, n, out)
		want := int64(1)
		if c == "scalar" {
			want = 0
		}
		if loads.Load() != want || records.Load() != want {
			panic("first tool count")
		}
		emit(map[string]any{"kind": "first-request", "engine": engine, "case": c, "rows": n,
			"init_begin_ns": initBegin, "ready_ns": ready, "prepared_ns": prepared, "executed_ns": executed,
			"stdout": out, "completed": true, "prepare_boundary": "Host read/adaptation of three bodies, native parent preload if applicable; no user code compilation",
			"execute_boundary": "unknown-source Python compile, per-request COW instance or fork/exit/reap (resident has neither), producer/JSON/transport/object conversion/scan/output; oracle excluded"})
		return
	}
	if mode == "cold" {
		for i := 0; i < reps; i++ {
			l, k := loads.Load(), records.Load()
			cp := cpu()
			t := time.Now()
			out := run(c, 17)
			wall, cost := time.Since(t).Nanoseconds(), cpu()-cp
			check(c, n, out)
			want := int64(1)
			if c == "scalar" {
				want = 0
			}
			if loads.Load()-l != want || records.Load()-k != want {
				panic("tool count")
			}
			emit(map[string]any{"kind": "speed", "engine": engine, "case": c, "rows": n, "mode": "cold", "sample": i, "wall_ns": wall, "cpu_ns": cost, "completed": true, "corpus": "ad-json-object"})
		}
		return
	}
	if mode == "hold" {
		snap := func(p string) {
			emit(map[string]any{"kind": "hold", "engine": engine, "case": c, "rows": n, "count": reps, "code": "runner-shared", "phase": p, "physical": physical(), "requested_traced_bytes": nil, "corpus": "ad-json-object"})
		}
		snap("startup")
		if native != nil && arm == "fork" {
			native.armEmpty(reps)
		}
		var wg sync.WaitGroup
		for i := 0; i < reps; i++ {
			wg.Add(1)
			go func() { defer wg.Done(); check(c, n, run(c, 17)) }()
		}
		if native != nil && arm == "fork" {
			native.waitEmpty(reps)
			snap("empty_tree")
			native.releaseEmpty()
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
		cp := cpu()
		t := time.Now()
		rows := make([]map[string]any, 40)
		outputs := make([]string, 40)
		var wg sync.WaitGroup
		for i := 0; i < 40; i++ {
			wg.Add(1)
			go func(i int) {
				defer wg.Done()
				s := shapes[schedule[i*17%40]]
				due := t.Add(time.Duration(i) * 500 * time.Microsecond)
				time.Sleep(time.Until(due))
				out := run(s.c, 17+s.n)
				latency := time.Since(due).Nanoseconds()
				outputs[i] = out
				rows[i] = map[string]any{"kind": "mixed", "engine": engine, "mix": c, "case": s.c, "rows": s.n, "mode": "mixed-cold", "sample": sample, "arrival": i, "delay_us": delay, "inflight": 40, "latency_ns": latency, "completed": true, "corpus": "ad-json-object"}
			}(i)
		}
		wg.Wait()
		wall, cost := time.Since(t).Nanoseconds(), cpu()-cp
		for i, out := range outputs {
			shape := shapes[schedule[i*17%40]]
			check(shape.c, shape.n, out)
		}
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
