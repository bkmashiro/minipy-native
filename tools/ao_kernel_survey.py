#!/usr/bin/env python3
"""AO bounded AST recheck. Never execute or export corpus source/values.

Reuse the private Z extractor unchanged (cutoff, sampling and replay dedup).
Counters are syntactic candidates, not type inference, CPU or compatibility.
"""
import argparse
import ast
from collections import Counter, defaultdict
import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
from typing import TypeGuard


def string_literal(n) -> TypeGuard[ast.Constant]:
    return isinstance(n, ast.Constant) and isinstance(n.value, str)


def static_path(n):
    if isinstance(n, ast.Name):
        return (n.id, ())
    if isinstance(n, ast.Subscript) and string_literal(n.slice):
        parent = static_path(n.value)
        if parent:
            return parent[0], parent[1] + (n.slice.value,)
    return None


def is_call(n, name):
    return isinstance(n, ast.Call) and isinstance(n.func, ast.Name) and n.func.id == name


def json_call(n, methods):
    return (isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)
            and isinstance(n.func.value, ast.Name) and n.func.value.id == 'json'
            and n.func.attr in methods)


def operations(code):
    tree = ast.parse(code)
    counts = Counter()
    parents = {child: node for node in ast.walk(tree) for child in ast.iter_child_nodes(node)}
    paths = Counter()
    siblings = defaultdict(set)
    for n in ast.walk(tree):
        if isinstance(n, ast.Subscript) and isinstance(n.ctx, ast.Load):
            path = static_path(n)
            if path and path[1]:
                counts['literal_key_read'] += 1
                paths[path] += 1
                siblings[(path[0], path[1][:-1])].add(path[1][-1])
                if len(path[1]) > 1:
                    counts['nested_literal_key_read'] += 1
                p = parents.get(n)
                if isinstance(p, ast.Assign) and p.value is n:
                    counts['literal_key_read_bound_to_name_or_target'] += 1
                if isinstance(p, ast.Call) and n in p.args:
                    counts['literal_key_read_passed_to_call'] += 1
                    if json_call(p, {'dumps', 'dump'}):
                        counts['literal_key_read_passed_to_json_encode'] += 1
        if json_call(n, {'loads', 'load'}):
            counts['json_decode_call'] += 1
        if json_call(n, {'dumps', 'dump'}):
            counts['json_encode_call'] += 1
        if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute):
            method = n.func.attr
            if method in {'startswith', 'endswith', 'split', 'rsplit', 'splitlines',
                          'partition', 'rpartition', 'find', 'rfind', 'index', 'join'}:
                counts['method_' + method + '_untyped'] += 1
                if string_literal(n.func.value):
                    counts['method_' + method + '_literal_receiver'] += 1
        if isinstance(n, ast.Compare):
            operands = [n.left] + n.comparators
            for i, op in enumerate(n.ops):
                if isinstance(op, (ast.In, ast.NotIn)):
                    counts['membership_untyped'] += 1
                    if string_literal(operands[i]):
                        counts['literal_needle_membership_untyped_haystack'] += 1
                    if string_literal(operands[i + 1]):
                        counts['membership_literal_string_haystack'] += 1
        if isinstance(n, (ast.ListComp, ast.SetComp, ast.DictComp, ast.GeneratorExp)):
            counts['comprehension'] += 1
            if any(g.ifs for g in n.generators):
                counts['filtered_comprehension'] += 1
            elt = n.value if isinstance(n, ast.DictComp) else n.elt
            if isinstance(elt, (ast.Attribute, ast.Subscript)):
                counts['field_projection_comprehension'] += 1
            if len(n.generators) == 1 and not n.generators[0].is_async:
                counts['single_sync_comprehension'] += 1
        if isinstance(n, ast.Call) and isinstance(n.func, ast.Name):
            name = n.func.id
            if name in {'filter', 'map', 'next', 'any', 'all', 'sum', 'min', 'max'}:
                counts['builtin_' + name + '_unresolved'] += 1
                if n.args and isinstance(n.args[0], ast.GeneratorExp):
                    counts['builtin_' + name + '_generator_untyped'] += 1
                    gen = n.args[0]
                    if name == 'sum':
                        if isinstance(gen.elt, (ast.Compare, ast.BoolOp)):
                            counts['sum_generator_predicate_syntax'] += 1
                        elif isinstance(gen.elt, ast.Constant) and type(gen.elt.value) is int:
                            counts['sum_generator_integer_literal_term'] += 1
                        elif isinstance(gen.elt, (ast.Attribute, ast.Subscript)):
                            counts['sum_generator_field_term_untyped'] += 1
                        else:
                            counts['sum_generator_other_term_untyped'] += 1
            if name == 'len' and n.args and isinstance(n.args[0], ast.ListComp):
                counts['len_listcomp_untyped'] += 1
        if isinstance(n, ast.AugAssign) and isinstance(n.op, ast.Add):
            p = parents.get(n)
            while p is not None and not isinstance(p, (ast.For, ast.While, ast.FunctionDef, ast.AsyncFunctionDef)):
                p = parents.get(p)
            if isinstance(p, (ast.For, ast.While)):
                counts['loop_augadd_untyped'] += 1
    counts['same_receiver_distinct_literal_siblings'] = sum(len(keys) >= 2 for keys in siblings.values())
    counts['repeated_literal_path'] = sum(freq > 1 for freq in paths.values())
    counts['json_decode_and_multi_key_syntax'] = int(bool(
        counts['json_decode_call'] and counts['same_receiver_distinct_literal_siblings']))
    # No type/alias/def-use proof: identical receiver syntax can denote unrelated
    # values across scopes, reassignments or iterations. Never label these JSON/i64.
    return +counts


def aggregate(items):
    return {'presence': dict(sorted(Counter(k for x in items for k in x).items())),
            'occurrences': dict(sorted(sum(items, Counter()).items()))}


def run(private_dir, output):
    extractor = private_dir / 'survey.py'
    original = json.loads((private_dir / 'summary.json').read_text())
    spec = importlib.util.spec_from_file_location('ao_private_survey', extractor)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    analyze = module.analyze
    group_summary = module.Group.summary
    collected = {}

    def analyze_extended(code):
        result = analyze(code)
        if 'error' not in result:
            result['ao'] = operations(code)
        return result

    def summarize(group):
        result = group_summary(group)
        result['ao'] = aggregate([x['ao'] for x in group.unique if 'ao' in x])
        # Hashes stay in memory and are not exported. Used only to verify global
        # source dedup and to reconcile source-family totals below.
        collected[id(group)] = {x['sha256']: x.get('ao', Counter()) for x in group.unique}
        return result

    setattr(module, 'analyze', analyze_extended)
    module.Group.summary = summarize
    old_argv = sys.argv
    # The extractor writes aggregates only. Keep its transient private metadata
    # inside our workspace and delete it even on failure.
    with tempfile.TemporaryDirectory(prefix='.ao-survey-', dir=output.parent) as tmp:
        sys.argv = [str(extractor), '--deadline-seconds', '600', '--output', str(Path(tmp) / 'aggregate.json')]
        try:
            with contextlib.redirect_stdout(io.StringIO()):
                module.main()
        finally:
            sys.argv = old_argv
        result = json.loads((Path(tmp) / 'aggregate.json').read_text())
    assert result['cutoff'] == original['cutoff']
    for key in ['message_high_watermark', 'sample_session_hashes', 'input_file_sha256', 'replay_dedup']:
        assert result['metadata'][key] == original['metadata'][key], key
    groups = {}
    for name, g in result['groups'].items():
        if not (name.startswith('hermes/') or name.startswith('appworld/')):
            continue
        old = original['groups'][name]
        assert g['unique_exact'] == old['unique_exact'], name
        assert g['raw']['scripts'] == old['raw']['scripts']
        assert g['truncated_sessions'] == 0
        groups[name] = {'submitted_scripts': g['raw']['scripts'],
                        'unique_scripts': g['unique_exact']['scripts'],
                        'parsed_scripts': g['unique_exact']['parsed'], **g['ao']}
    # Group summary is called in insertion order by the unchanged extractor.
    all_groups = dict(zip(result['groups'], collected.values(), strict=True))
    families = {}
    main = {}
    for prefix in ['hermes/', 'appworld/']:
        unique = {}
        for name, scripts in all_groups.items():
            if name.startswith(prefix):
                unique.update(scripts)
        families[prefix[:-1]] = {'unique_scripts': len(unique), **aggregate(list(unique.values()))}
        main.update(unique)
    assert len(main) == result['cross_source_exact_unique_main'] == original['cross_source_exact_unique_main'] == 632
    final = {'schema': 1, 'cutoff': original['cutoff'],
             'extractor_sha256': hashlib.sha256(extractor.read_bytes()).hexdigest(),
             'analyzer_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
             'original_summary_sha256': hashlib.sha256((private_dir / 'summary.json').read_bytes()).hexdigest(),
             'sampling_and_original_aggregates_exact_match': True,
             'definitions': 'Syntactic presence per exact-source-deduplicated script. Overlapping categories. No type, JSON provenance, alias, builtin binding, execution CPU or compatibility inference.',
             'groups': groups, 'families': families,
             'main': {'unique_scripts': len(main), **aggregate(list(main.values()))}}
    output.write_text(json.dumps(final, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(final['families'], indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--private-dir', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    run(args.private_dir, args.output)
