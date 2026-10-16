#!/usr/bin/env python3
"""DI: bounded controlled replay. Explicit output path; never auto-run private cases."""
import argparse
import ast
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
LOCK = Path.home() / '.hermes/cache/scratch/minipy-perf-build.lock'


def inventory(source):
    """Syntactic site denominators, including rejected/unsupported tails."""
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return None
    nodes = list(ast.walk(tree))
    call_functions = {id(n.func) for n in nodes if isinstance(n, ast.Call)}
    tool_calls = sum(isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)
                     and isinstance(n.func.value, ast.Name) and n.func.value.id == 'tools'
                     for n in nodes)
    selectors = sum(isinstance(n, ast.Subscript) or
                    (isinstance(n, ast.Attribute) and id(n) not in call_functions) or
                    (isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)
                     and n.func.attr == 'get') for n in nodes)
    return {'tool_call_sites': tool_calls, 'data_selector_sites': selectors}


def decorate(row: dict[str, Any], case: dict[str, Any]) -> dict[str, Any]:
    row.setdefault('case_id', case.get('case_id'))
    sites = inventory(case['source'])
    # Admission proves binding against the supplied ABI, not completeness of a real Host mapping.
    complete = case.get('contract_complete') is True
    bound = sites if row.get('whole_source_admitted') and complete else None
    row['static_binding'] = {'source_sites': sites, 'whole_program_bound_sites': bound,
                             'contract_complete': case.get('contract_complete'),
                             'oracle_complete': case.get('oracle_complete')}
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--mode', choices=['timing', 'diagnostic'], required=True)
    parser.add_argument('--backend', choices=['tape', 'native', 'both'], default='both')
    args = parser.parse_args()
    os.chdir(ROOT)
    data = args.input.read_bytes()
    if len(data) > 64 * 1024 * 1024:
        raise ValueError('bounded runner: input exceeds 64 MiB')
    cases = [json.loads(line) for line in data.splitlines() if line.strip()]
    if not 1 <= len(cases) <= 256:
        raise ValueError('bounded runner: 1..256 cases required')
    if any(not isinstance(c.get('case_id'), str) or not isinstance(c.get('source'), str) for c in cases):
        raise ValueError('case_id and source must be strings')
    ids = [c['case_id'] for c in cases]
    if len(set(ids)) != len(ids):
        raise ValueError('duplicate case_id')
    if args.output.exists():
        raise FileExistsError('retain previous attempts: choose a new output path')
    target = ROOT / 'target'
    if not target.is_dir() or not str(target.resolve()).startswith('/Volumes/') or not os.path.ismount('/Volumes/M.2'):
        raise RuntimeError('existing mounted external target required')
    env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS='1', RUSTUP_AUTO_INSTALL='0')
    if env.get('RUSTFLAGS') or env.get('CARGO_ENCODED_RUSTFLAGS'):
        raise RuntimeError('unexpected compiler flags: use ordinary release')
    command = ['cargo', '+1.96.0', 'build', '--offline', '--locked', '--release', '--features', 'tool-json', '--example', 'di_corpus']
    sources = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', Path(__file__).resolve(), ROOT / 'examples/di_corpus.rs', *sorted((ROOT / 'src').rglob('*.rs'))]
    identity = {'kind': 'manifest', 'mode': args.mode, 'case_count': len(cases),
                'input_sha256': hashlib.sha256(data).hexdigest(),
                'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
                'source_sha256': {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sources},
                'rustc': subprocess.check_output(['rustc', '+1.96.0', '-Vv'], text=True),
                'platform': platform.platform(), 'command': command,
                'target': str(target.resolve()), 'runtime_flags': 'ordinary release; no tracing in timing mode'}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    # Entire build/run serialized against the existing research compute lock.
    with LOCK.open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        subprocess.run(command, env=env, check=True)
        binary = target / 'release/examples/di_corpus'
        identity['binary_sha256'] = hashlib.sha256(binary.read_bytes()).hexdigest()
        with args.output.open('x') as output:
            output.write(json.dumps(identity) + '\n')
            for backend in (['tape', 'native'] if args.backend == 'both' else [args.backend]):
                # One resident process per backend. New prepare/execute/drop per input line.
                try:
                    result = subprocess.run([str(binary), backend, args.mode], input=data.decode(),
                                            env=env, text=True, capture_output=True, timeout=600)
                except subprocess.TimeoutExpired as error:
                    output.write(json.dumps({'kind': 'process_error', 'backend': backend,
                                             'error': 'timeout', 'stdout': (error.stdout or b'').decode(),
                                             'stderr': (error.stderr or b'').decode()}) + '\n')
                    raise
                if result.returncode:
                    output.write(json.dumps({'kind': 'process_error', 'backend': backend,
                                             'returncode': result.returncode, 'stdout': result.stdout,
                                             'stderr': result.stderr}) + '\n')
                    raise RuntimeError(f'runner failed: {result.returncode}; retained in output')
                rows = [json.loads(line) for line in result.stdout.splitlines()]
                if len(rows) != len(cases) + 1 or rows[0].get('kind') != 'setup':
                    raise RuntimeError('runner row count/setup mismatch')
                output.write(json.dumps(dict(rows[0], backend=backend)) + '\n')
                for row, case in zip(rows[1:], cases):
                    output.write(json.dumps(decorate(row, case), ensure_ascii=False) + '\n')
                output.flush()
    print(f'{len(cases)} cases x {2 if args.backend == "both" else 1} backend(s): {args.output.resolve()}')


if __name__ == '__main__':
    main()
