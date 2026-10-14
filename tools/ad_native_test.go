package main

import (
	"context"
	"encoding/json"
	"fmt"
	pysolate "github.com/bkmashiro/agent-python-runtime"
	"os"
	"sync/atomic"
	"testing"
)

func TestOracle(t *testing.T) {
	for _, c := range []string{"weighted", "stateful"} {
		if got := oracle(c, 0); got != "17\n" {
			t.Fatal(got)
		}
	}
	if oracle("scalar", 0) != "734\nTrue\n" {
		t.Fatal("scalar")
	}
	if oracle("weighted", 100) == oracle("stateful", 100) {
		t.Fatal("stateful predicate lost")
	}
}
func TestPhysicalSum(t *testing.T) {
	out := treePhysical()
	var pss, rss int64
	for _, p := range out["per_pid"].([]map[string]any) {
		pss += p["pss_bytes"].(int64)
		rss += p["rss_bytes"].(int64)
	}
	if out["pss_bytes"] != pss || out["rss_bytes"] != rss {
		t.Fatal(out)
	}
}
func TestNativeRequestLifecycleAndJSON(t *testing.T) {
	if os.Getenv("AD_NATIVE_SCRIPT") == "" {
		t.Fatal("real native script required")
	}
	for _, arm := range []string{"fork", "resident"} {
		var loads, records atomic.Int64
		manifest := pysolate.Manifest{
			"load": {Call: func(ctx context.Context, raw json.RawMessage) (any, error) {
				loads.Add(1)
				return []map[string]int64{{"quantity": 3, "unit_price_cents": 11}}, nil
			}},
			"record": {Call: func(ctx context.Context, raw json.RawMessage) (any, error) {
				var a map[string]int64
				if e := json.Unmarshal(raw, &a); e != nil {
					return nil, e
				}
				records.Add(1)
				return a["total"] + 17, nil
			}},
		}
		r := startNative(manifest, arm, false)
		native = r
		body := "from types import SimpleNamespace\nquotes = [SimpleNamespace(**q) for q in tools.load_quotes(batch_id=17)]\nprint(tools.record_total(batch_id=17,total=quotes[0].quantity*quotes[0].unit_price_cents))\nprint(__import__('os').getpid())\n"
		r.configure(map[string]string{"probe": body})
		seen := map[int]bool{}
		before := r.cpu()
		for i := 0; i < 3; i++ {
			out, e := r.run(body, map[string]any{})
			if e != nil {
				t.Fatal(e)
			}
			var value, pid int
			if _, e = fmt.Sscanf(out, "%d\n%d\n", &value, &pid); e != nil || value != 50 {
				t.Fatal(out, e)
			}
			if arm == "fork" && (seen[pid] || pid == r.cmd.Process.Pid) {
				t.Fatal("fork worker reused", pid)
			}
			if arm == "resident" && pid != r.cmd.Process.Pid {
				t.Fatal("resident spawned", pid)
			}
			seen[pid] = true
			var children int
			json.Unmarshal(r.rpc(map[string]any{"op": "cpu"})["children"], &children)
			if children != 0 {
				t.Fatal("request returned before reap", children)
			}
		}
		if r.cpu() <= before || loads.Load() != 3 || records.Load() != 3 {
			t.Fatal("CPU/tool accounting", loads.Load(), records.Load())
		}
		r.close()
		native = nil
	}
}

func TestChildCPUResponseOrder(t *testing.T) {
	r := &nativeRunner{}
	for _, cost := range []int64{200, 100, 300, 250} {
		r.observeChildCPU(cost)
	}
	if r.childCPU.Load() != 300 {
		t.Fatal("cumulative child CPU went backwards")
	}
}

func TestNativeKernelCPUAndReapedChildCPU(t *testing.T) {
	r := startNative(pysolate.Manifest{}, "fork", false)
	defer r.close()
	body := "x=0\nfor i in range(500000): x+=i\nprint(x)\n"
	r.configure(map[string]string{"probe": body})
	original := r.address
	// Sampling must still work when its control socket is unavailable.
	r.address = "127.0.0.1:1"
	before := r.cpu()
	r.address = original
	out, e := r.run(body, map[string]any{})
	if e != nil || out != "124999750000\n" {
		t.Fatal(out, e)
	}
	if r.childCPU.Load() < 1000000 {
		t.Fatal("reaped child CPU missing", r.childCPU.Load())
	}
	sampled := r.cpu()
	if sampled <= before || sampled < r.childCPU.Load() {
		t.Fatal("CPU sum", sampled)
	}
	var diagnostic int64
	json.Unmarshal(r.rpc(map[string]any{"op": "cpu"})["cpu_ns"], &diagnostic)
	difference := diagnostic - sampled
	if difference < -2000000 || difference > 2000000 {
		t.Fatal("kernel clock disagrees with native self+children RUSAGE", sampled, diagnostic)
	}
	t.Logf("kernel parent + child: %d ns; diagnostic RUSAGE: %d ns; reaped child: %d ns", sampled, diagnostic, r.childCPU.Load())
}
