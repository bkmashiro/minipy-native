#!/usr/bin/env python3
"""Static shape survey of BI's private CLI sample; only aggregate labels leave memory."""
import ast
from collections import Counter
import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import symtable
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
    old_argv = sys.argv
    sys.argv = [str(private / 'survey.py'), '--output', str(Path(directory) / 'summary.json'), '--deadline-seconds', '600']
    try:
        with contextlib.redirect_stdout(io.StringIO()):
            module.main()
    finally:
        sys.argv = old_argv
    actual = json.loads((Path(directory) / 'summary.json').read_text())
    expected = json.loads((private / 'summary.json').read_text())
    assert actual['metadata']['sample_session_hashes']['cli'] == expected['metadata']['sample_session_hashes']['cli']
    assert actual['metadata']['replay_dedup']['cli'] == expected['metadata']['replay_dedup']['cli']
    assert actual['groups']['hermes/cli']['unique_exact'] == expected['groups']['hermes/cli']['unique_exact']
    assert len(seen) == 520

sample = sorted(seen)[:64]
stdlib = set(sys.stdlib_module_names) | set(sys.builtin_module_names)
os_modules = {'os', 'subprocess', 'pathlib', 'shutil', 'socket', 'glob', 'tempfile', 'platform', 'signal', 'ctypes', 'http', 'urllib'}
host_modules = {'hermes_tools', 'agent_helpers'}
host_names = {'terminal', 'web_search', 'web_extract', 'read_file', 'write_file', 'patch', 'search_files', 'new_tab', 'goto_url', 'js', 'cdp', 'capture_screenshot', 'fill_input', 'page_info', 'wait_for_load'}
allowed_names = set(dir(__builtins__)) | {'True', 'False', 'None'}
missing_ops = {
    ast.Try: 'try_except', ast.With: 'with', ast.AsyncWith: 'async_with',
    ast.FunctionDef: 'def', ast.AsyncFunctionDef: 'async_def', ast.ClassDef: 'class',
    ast.List: 'list', ast.Tuple: 'tuple', ast.Dict: 'dict', ast.Set: 'set',
    ast.ListComp: 'list_comp', ast.DictComp: 'dict_comp', ast.SetComp: 'set_comp', ast.GeneratorExp: 'generator',
    ast.Subscript: 'subscript', ast.JoinedStr: 'f_string', ast.Lambda: 'lambda',
    ast.While: 'while', ast.Raise: 'raise', ast.Delete: 'delete', ast.Assert: 'assert',
    ast.Yield: 'yield', ast.YieldFrom: 'yield_from', ast.Await: 'await',
}
counts = Counter()
shapes = Counter()
small_shapes = Counter()
for digest in sample:
    source = seen[digest]
    try:
        tree = ast.parse(source)
        table = symtable.symtable(source, '<private>', 'exec')
    except (SyntaxError, ValueError):
        counts['unparseable'] += 1
        continue
    modules = {alias.name.split('.')[0] for node in ast.walk(tree) if isinstance(node, (ast.Import, ast.ImportFrom)) for alias in (node.names if isinstance(node, ast.Import) else [ast.alias(name=node.module or '')])}
    tags = set()
    if modules & host_modules:
        tags.add('explicit_host_import')
    if modules & os_modules:
        tags.add('stdlib_os_io_dependency')
    if modules - stdlib - host_modules:
        tags.add('nonstdlib_module_dependency')
    if modules:
        tags.add('any_import')
    for node in ast.walk(tree):
        for cls, label in missing_ops.items():
            if isinstance(node, cls):
                tags.add('language_or_data_feature')
                tags.add('op_' + label)
        if isinstance(node, ast.Call):
            if not isinstance(node.func, ast.Name) or node.func.id not in {'print', 'len', 'abs', 'min', 'max'}:
                tags.add('non_bi_call_shape')
            if isinstance(node.func, ast.Name) and node.func.id in host_names:
                tags.add('known_host_global')
            if node.keywords:
                tags.add('keyword_call')
    top = table.get_symbols()
    globals_needed = {s.get_name() for s in top if s.is_referenced() and not s.is_assigned() and not s.is_imported() and s.get_name() not in allowed_names}
    if globals_needed & host_names:
        tags.add('known_host_global')
    if globals_needed - host_names:
        tags.add('unresolved_global_needs_contract')
    for tag in tags:
        counts[tag] += 1
    ops = tuple(sorted(t[3:] for t in tags if t.startswith('op_')))
    if ops in ((), ('f_string',)):
        small_shapes[(ops, 'known_host_global' in tags or 'explicit_host_import' in tags, 'keyword_call' in tags, 'unresolved_global_needs_contract' in tags)] += 1
    shapes[(bool({'explicit_host_import', 'known_host_global'} & tags), bool({'stdlib_os_io_dependency', 'nonstdlib_module_dependency'} & tags), 'language_or_data_feature' in tags)] += 1
print(json.dumps({'n': len(sample), 'ast_parsed': len(sample) - counts['unparseable'], 'features_per_script': dict(sorted(counts.items())), 'three_axes_host_dependency_language': {str(k): v for k, v in sorted(shapes.items())}, 'small_shapes': {str(k): v for k, v in sorted(small_shapes.items())}, 'labels_only': True}, sort_keys=True))
