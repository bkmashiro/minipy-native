// AD native bridge. Go Tool producer, JSON envelope and generation are shared
// with the Pysolate Manifest; native transport is real localhost TCP/JSON.
package main

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	pysolate "github.com/bkmashiro/agent-python-runtime"
	"golang.org/x/sys/unix"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync/atomic"
	"time"
)

type nativeRunner struct {
	cmd        *exec.Cmd
	address    string
	server     net.Listener
	empty      chan struct{}
	emptyCount atomic.Int64
	childCPU   atomic.Int64
	sources    map[string]string
}

func startNative(manifest pysolate.Manifest, arm string, hold bool) *nativeRunner {
	ln, e := net.Listen("tcp", "127.0.0.1:0")
	if e != nil {
		panic(e)
	}
	r := &nativeRunner{server: ln}
	go func() {
		for {
			conn, e := ln.Accept()
			if e != nil {
				return
			}
			go func() {
				defer conn.Close()
				conn.SetDeadline(time.Now().Add(10 * time.Minute))
				var req struct {
					Tool string          `json:"tool"`
					Args json.RawMessage `json:"args"`
				}
				if e := json.NewDecoder(conn).Decode(&req); e != nil {
					panic(e)
				}
				var value any
				var err error
				if req.Tool == "empty" {
					r.emptyCount.Add(1)
					<-r.empty
					value = true
				} else if f, ok := manifest[req.Tool]; ok {
					value, err = f.Call(context.Background(), req.Args)
				} else {
					err = fmt.Errorf("unknown tool %s", req.Tool)
				}
				out := map[string]any{"value": value}
				if err != nil {
					out = map[string]any{"error": err.Error()}
				}
				// Same compact {"value":...} envelope as the Pysolate Tool ABI.
				if e := json.NewEncoder(conn).Encode(out); e != nil {
					panic(e)
				}
			}()
		}
	}()
	script := os.Getenv("AD_NATIVE_SCRIPT")
	if script == "" {
		panic("AD_NATIVE_SCRIPT required")
	}
	port := ln.Addr().(*net.TCPAddr).Port
	r.cmd = exec.Command("python3", "-I", script, arm, "127.0.0.1", strconv.Itoa(port))
	r.cmd.Stderr = os.Stderr
	out, e := r.cmd.StdoutPipe()
	if e != nil {
		panic(e)
	}
	if e = r.cmd.Start(); e != nil {
		panic(e)
	}
	var ready struct {
		Port    int
		PID     int
		Version string
	}
	if e = json.NewDecoder(out).Decode(&ready); e != nil {
		panic(e)
	}
	if ready.PID != r.cmd.Process.Pid {
		panic("native pid")
	}
	r.address = fmt.Sprintf("127.0.0.1:%d", ready.Port)
	return r
}
func (r *nativeRunner) rpc(req any) map[string]json.RawMessage {
	c, e := net.DialTimeout("tcp", r.address, 10*time.Second)
	if e != nil {
		panic(e)
	}
	defer c.Close()
	c.SetDeadline(time.Now().Add(10 * time.Minute))
	if e = json.NewEncoder(c).Encode(req); e != nil {
		panic(e)
	}
	var out map[string]json.RawMessage
	if e = json.NewDecoder(bufio.NewReader(c)).Decode(&out); e != nil {
		panic(e)
	}
	if v, ok := out["error"]; ok {
		panic(string(v))
	}
	return out
}
func (r *nativeRunner) configure(sources map[string]string) {
	r.sources = sources
	r.rpc(map[string]any{"op": "sources", "sources": sources})
}
func (r *nativeRunner) run(s string, in map[string]any) (string, error) {
	var name string
	for c, body := range r.sources {
		if body == s {
			name = c
			break
		}
	}
	if name == "" {
		return "", fmt.Errorf("unknown body")
	}
	out := r.rpc(map[string]any{"op": "run", "case": name, "inputs": in})
	var childCost int64
	if e := json.Unmarshal(out["children_cpu_ns"], &childCost); e != nil {
		return "", e
	}
	r.observeChildCPU(childCost)
	var stdout string
	if e := json.Unmarshal(out["stdout"], &stdout); e != nil {
		return "", e
	}
	return stdout, nil
}
func (r *nativeRunner) observeChildCPU(cost int64) {
	if cost < 0 {
		panic("negative child CPU")
	}
	for {
		previous := r.childCPU.Load()
		if cost <= previous || r.childCPU.CompareAndSwap(previous, cost) {
			return
		}
	}
}
func (r *nativeRunner) parentCPU() int64 {
	// Linux process CPUCLOCK_SCHED ABI, equivalent to clock_getcpuclockid(pid).
	// Per-process (not per-thread), includes user+system CPU, nanosecond clock.
	// Read from the kernel; never send a sampling RPC across the timed boundary.
	clockID := int32(^r.cmd.Process.Pid)<<3 | 2
	var ts unix.Timespec
	if e := unix.ClockGettime(clockID, &ts); e != nil {
		panic(e)
	}
	return ts.Nano()
}
func (r *nativeRunner) cpu() int64 { return r.parentCPU() + r.childCPU.Load() }
func (r *nativeRunner) close() {
	out := r.rpc(map[string]any{"op": "cpu"})
	var children int
	if e := json.Unmarshal(out["children"], &children); e != nil {
		panic(e)
	}
	if children != 0 {
		panic("unreaped native children")
	}
	r.cmd.Process.Kill()
	r.cmd.Wait()
	r.server.Close()
}
func (r *nativeRunner) armEmpty(count int) {
	r.empty = make(chan struct{})
	r.rpc(map[string]any{"op": "empty", "enabled": true})
}
func (r *nativeRunner) waitEmpty(count int) {
	due := time.Now().Add(5 * time.Minute)
	for r.emptyCount.Load() != int64(count) {
		if time.Now().After(due) {
			panic("empty hold timeout")
		}
		time.Sleep(time.Millisecond)
	}
}
func (r *nativeRunner) releaseEmpty() { close(r.empty) }
func treePhysical() map[string]any {
	pids := []int{os.Getpid()}
	if native != nil {
		pids = append(pids, native.cmd.Process.Pid)
		for i := 1; i < len(pids); i++ {
			files, e := filepath.Glob(fmt.Sprintf("/proc/%d/task/*/children", pids[i]))
			if e != nil {
				panic(e)
			}
			seen := map[int]bool{}
			for _, f := range files {
				b, e := os.ReadFile(f)
				if e != nil {
					panic(e)
				}
				for _, v := range strings.Fields(string(b)) {
					pid, e := strconv.Atoi(v)
					if e != nil {
						panic(e)
					}
					if !seen[pid] {
						pids = append(pids, pid)
						seen[pid] = true
					}
				}
			}
		}
	}
	var rss, pss int64
	per := []map[string]any{}
	for _, pid := range pids {
		m := pidPhysical(pid)
		rss += m["rss_bytes"]
		pss += m["pss_bytes"]
		per = append(per, map[string]any{"pid": pid, "rss_bytes": m["rss_bytes"], "pss_bytes": m["pss_bytes"]})
	}
	return map[string]any{"rss_bytes": rss, "pss_bytes": pss, "per_pid": per, "tree_pids": pids}
}
