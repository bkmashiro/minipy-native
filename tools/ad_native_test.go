package main

import (
	"context"
	"encoding/json"
	"fmt"
	pysolate "github.com/bkmashiro/agent-python-runtime"
	"os"
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
		loads, records := 0, 0
		manifest := pysolate.Manifest{
			"load": {Call: func(ctx context.Context, raw json.RawMessage) (any, error) {
				loads++
				return []map[string]int64{{"quantity": 3, "unit_price_cents": 11}}, nil
			}},
			"record": {Call: func(ctx context.Context, raw json.RawMessage) (any, error) {
				var a map[string]int64
				if e := json.Unmarshal(raw, &a); e != nil {
					return nil, e
				}
				records++
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
		if r.cpu() <= before || loads != 3 || records != 3 {
			t.Fatal("CPU/tool accounting", loads, records)
		}
		r.close()
		native = nil
	}
}
