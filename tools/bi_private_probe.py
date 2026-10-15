#!/usr/bin/env python3
"""Ephemeral read-only private admission probe; emits aggregate counters only."""
import contextlib
from collections import Counter
import fcntl
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

private = Path.home() / '.hermes/private-research/minipy-trajectory-survey'
spec = importlib.util.spec_from_file_location('private_survey', private / 'survey.py')
assert spec is not None and spec.loader is not None
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
original_code = module.Group.code
seen = {}
first_group = None

def collect(self, code, origin):
    global first_group
    if first_group is None:
        first_group = self
    original_code(self, code, origin)
    if self is first_group and origin == 'execute_code':
        seen.setdefault(hashlib.sha256(code.encode()).hexdigest(), code)
module.Group.code = collect
with tempfile.TemporaryDirectory(dir=Path.home() / '.hermes/cache/scratch') as directory:
    old = sys.argv
    sys.argv = [str(private / 'survey.py'), '--output', str(Path(directory) / 'summary.json'), '--deadline-seconds', '600']
    try:
        with contextlib.redirect_stdout(io.StringIO()):
            module.main()
    finally:
        sys.argv = old
    actual = json.loads((Path(directory) / 'summary.json').read_text())
    expected = json.loads((private / 'summary.json').read_text())
    assert actual['metadata']['sample_session_hashes']['cli'] == expected['metadata']['sample_session_hashes']['cli']
    assert actual['metadata']['replay_dedup']['cli'] == expected['metadata']['replay_dedup']['cli']
    assert actual['groups']['hermes/cli']['unique_exact'] == expected['groups']['hermes/cli']['unique_exact']
    assert len(seen) == 520
    sample = sorted(seen)[:64]
    payload = ''.join(json.dumps(seen[digest], ensure_ascii=False) + '\n' for digest in sample)
    assert len(payload.encode()) < 1_000_000
    env = os.environ.copy()
    env.update(CARGO_TARGET_DIR='/Users/yuzhe/projects/minipy-native/target', CARGO_BUILD_JOBS='1')
    with open(Path.home() / '.hermes/cache/scratch/minipy-perf-build.lock', 'w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        result = subprocess.run(['cargo', '+1.96.0', 'run', '--offline', '--locked', '--quiet', '--features', 'tool-json', '--example', 'bi_private_probe'], input=payload, text=True, capture_output=True, env=env, check=True)
    labels = result.stdout.splitlines()
    assert len(labels) == len(sample), (len(labels), len(sample))
    print(json.dumps({'sample_rule': 'lowest 64 SHA256 of 520 exact-unique Hermes/cli execute_code strings, unchanged Z cutoff/replay/time-quantile sampling', 'count': len(sample), 'outcomes': dict(sorted(Counter(labels).items())), 'cli_aggregate_reconciled': True, 'other_strata': 'not comparable: new pre-cutoff subagent sessions changed time-quantile sample', 'probe': 'InterpreterUnit::prepare_script(source, &[]) with tool-json; no execution; no Host schema', 'source_exported': False}, sort_keys=True))
