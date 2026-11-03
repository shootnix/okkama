#!/usr/bin/env python3
"""Compare ABI/intrinsic registries with phase-0 inventory; never run programs.

Native support comes from a supplied --native-capabilities report. Intrinsic
locations are source evidence only, not a semantic or execution coverage claim.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[3]


def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as source:
        for chunk in iter(lambda: source.read(65536), b''):
            result.update(chunk)
    return result.hexdigest()


def index(rows, field):
    result = {}
    for row in rows:
        key = row[field]
        if key in result:
            raise ValueError(f'duplicate {field}: {key}')
        result[key] = row
    return result


def compare_registry(expected, current, fields):
    old, new = index(expected, 'symbol'), index(current, 'symbol')
    return {
        'removed': sorted(old.keys() - new.keys()),
        'added': sorted(new.keys() - old.keys()),
        'changed': [
            {'symbol': symbol, 'fields': [field for field in fields
                                         if old[symbol][field] != new[symbol][field]]}
            for symbol in sorted(old.keys() & new.keys())
            if any(old[symbol][field] != new[symbol][field] for field in fields)
        ],
    }


def validate_capabilities(natives, entries):
    declared, reported = index(natives, 'symbol'), index(entries, 'symbol')
    if declared.keys() != reported.keys():
        raise ValueError('native capability report does not match the current ABI registry')
    if any(type(row.get('supported')) is not bool for row in entries):
        raise ValueError('native supported flags must be JSON booleans')
    return reported


def audit(inventory_path, report_path):
    inventory = json.loads(inventory_path.read_text())
    abi_path = ROOT / 'okkama-rs/crates/okkama-runtime-abi/src/table.rs'
    compiler_path = ROOT / 'okkama-rs/crates/okkama-compiler/src/ir/mod.rs'
    native_rows = [dict(name=m[1], id=int(m[2]), symbol=m[3], signature=m[4])
                   for m in re.finditer(r'^    (\w+) = (\d+), "([^"]+)", "([^"]+)";',
                                        abi_path.read_text(), re.M)]
    if not native_rows:
        raise ValueError('native ABI parser found no declarations')
    index(native_rows, 'id')
    index(inventory['natives'], 'id')
    native_drift = compare_registry(inventory['natives'], native_rows, ['name', 'id', 'signature'])
    table = re.search(r'pub const INTRINSICS: \[&str; (\d+)\] = \[(.*?)\];',
                      compiler_path.read_text(), re.S)
    if table is None:
        raise ValueError('compiler intrinsic registry was not found')
    intrinsic_rows = [dict(symbol=s) for s in re.findall(r'"([^"]+)"', table[2])]
    if len(intrinsic_rows) != int(table[1]):
        raise ValueError('intrinsic registry length disagrees with its declaration')
    intrinsic_drift = compare_registry(inventory['intrinsics'], intrinsic_rows, [])
    report = json.loads(report_path.read_text())
    entries = report if isinstance(report, list) else report['entries']
    supported = validate_capabilities(native_rows, entries)

    locations = {row['symbol']: [] for row in intrinsic_rows}
    source_hashes = {}
    for path in sorted((ROOT / 'okkama-rs/crates/sparrow-engine/src').rglob('*.rs')):
        relative = str(path.relative_to(ROOT))
        source_hashes[relative] = digest(path)
        source = path.read_text().split('#[cfg(test)]', 1)[0]
        for line, text in enumerate(source.splitlines(), 1):
            for symbol in re.findall(r'"(okkama\.[^"\n]+)"', text):
                if symbol in locations:
                    locations[symbol].append({'path': relative, 'line': line})

    missing_literals = sorted(symbol for symbol, sites in locations.items() if not sites)
    drift = any(native_drift.values()) or any(intrinsic_drift.values())
    return {
        'scope': 'registry drift and declared adapter support; not semantic conformance',
        'status': 'REVIEW_REQUIRED' if drift or missing_literals else 'REGISTRIES_MATCH',
        'inventory_sha256': digest(inventory_path),
        'native_report_sha256': digest(report_path),
        'reported_runtime_sha256': report.get('runtime_sha256') if isinstance(report, dict) else None,
        'source_hashes': {str(abi_path.relative_to(ROOT)): digest(abi_path),
                          str(compiler_path.relative_to(ROOT)): digest(compiler_path), **source_hashes},
        'native_drift': native_drift,
        'intrinsic_drift': intrinsic_drift,
        'counts': {
            'natives': len(native_rows),
            'native_declared_supported': sum(row['supported'] for row in entries),
            'intrinsics': len(intrinsic_rows),
            'intrinsics_with_source_literal': len(locations) - len(missing_literals),
        },
        'natives': [{**row, 'declared_supported': supported[row['symbol']]['supported'],
                     'semantic_validation': 'not_assessed'} for row in native_rows],
        'intrinsics': [{'symbol': symbol, 'source_locations': sites,
                        'semantic_validation': 'not_assessed'} for symbol, sites in sorted(locations.items())],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inventory', type=Path,
                        default=ROOT / 'okkama-rs/docs/new-runtime/inventory.json')
    parser.add_argument('--native-report', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    report = audit(args.inventory, args.native_report)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'status': report['status'], **report['counts']}))
    return 0 if report['status'] == 'REGISTRIES_MATCH' else 1


if __name__ == '__main__':
    raise SystemExit(main())
