#!/usr/bin/env python3
"""Read-only evidence join. Never compile or execute inventoried programs."""
import collections
import gzip
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
DOCS = ROOT / 'okkama-rs/docs/new-runtime'


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(65536), b''):
            h.update(block)
    return h.hexdigest()


def index_unique(rows, key):
    result = {}
    for row in rows:
        value = row[key]
        if value in result:
            raise ValueError(f'duplicate {key}: {value}')
        result[value] = row
    return result


def contract_review(inventory_contracts, definitions):
    """Join by ID, preserving status/aspects; never infer coverage from a title."""
    indexed = index_unique(definitions, 'id')
    index_unique(inventory_contracts, 'id')
    result = []
    for contract in inventory_contracts:
        definition = indexed.get(contract['id'], {})
        status = definition.get('status', 'unknown')
        if status == 'mandatory':
            action = 'review_required_aspects' if contract['cases'] else 'missing_case_mapping'
        elif status == 'unresolved':
            action = 'unresolved_blocks_release'
        elif status in ('deferred', 'informative'):
            action = 'tracked_not_mandatory'
        else:
            action = 'unknown_status_requires_review'
        result.append(dict(contract, status=status, aspects=definition.get('aspects', {}),
                           discrepancies=definition.get('discrepancies', []),
                           definition_source=definition.get('definition_source'),
                           review_action=action))
    return result


def historical_result(record, matches):
    if record is None:
        return 'absent'
    if not matches:
        return 'source_mismatch'
    return 'historical_pass' if record.get('status') == 'PASS' else 'historical_nonpass'


def runtime_evidence():
    """Compare saved results with current inputs; do not promote them to current PASS."""
    ocs_path = DOCS / 'baseline/conformance-results.jsonl.gz'
    std_path = DOCS / 'baseline/stdlib.json'
    with gzip.open(ocs_path, 'rt') as stream:
        saved_cases = index_unique((json.loads(line) for line in stream if line.strip()), 'id')
    cases = []
    for path in sorted((ROOT / 'conformance/cases').rglob('case.json')):
        case = json.loads(path.read_text())
        saved = saved_cases.get(case['id'])
        hashes = {}
        for source in sorted(path.parent.rglob('*')):
            if source.is_symlink():
                hashes[str(source.relative_to(path.parent))] = 'symlink:' + str(source.readlink())
            elif source.is_file():
                hashes[str(source.relative_to(path.parent))] = digest(source)
        cases.append(dict(id=case['id'], mode=case['mode'],
                          path=str(path.relative_to(ROOT)),
                          evidence=historical_result(saved, saved is not None and
                                                     hashes == saved.get('source_sha256'))))
    index_unique(cases, 'id')
    std = json.loads(std_path.read_text())
    index_unique(std['results'], 'path')
    modules = []
    for saved in std['results']:
        path = (ROOT / saved['path']).resolve()
        if ROOT not in path.parents:
            raise ValueError(f'path outside repository: {path}')
        matches = path.is_file() and digest(path) == saved['sha256']
        # Exact int: bool/str must never masquerade as process exit status zero.
        passed = type(saved.get('code')) is int and saved['code'] == 0
        modules.append(dict(path=saved['path'], evidence=historical_result(
            {'status': 'PASS' if passed else 'NONPASS'}, matches)))
    return dict(
        scope='Historical origin only; case hashes include expectations and support files. '
              'Stdlib rows match module source only, not its dependency closure. '
              'Neither proves current new-runtime compatibility or current environment equivalence.',
        input_sha256={'ocs_results': digest(ocs_path), 'stdlib_results': digest(std_path)},
        ocs_summary=dict(collections.Counter(row['evidence'] for row in cases)),
        stdlib_summary=dict(collections.Counter(row['evidence'] for row in modules)),
        removed_case_ids=sorted(set(saved_cases) - {c['id'] for c in cases}),
        ocs_cases=cases, stdlib_modules=modules)


def main():
    inventory_path = DOCS / 'inventory.json'
    baseline_path = DOCS / 'baseline/source-checks.json'
    inventory = json.loads(inventory_path.read_text())
    baseline = json.loads(baseline_path.read_text())
    programs = index_unique(inventory['programs'], 'path')
    checks = index_unique(baseline['results'], 'path')
    definitions = []
    definition_hashes = {}
    for path in sorted((ROOT / 'conformance/contracts').glob('*.json')):
        document = json.loads(path.read_text())
        if document.get('schema') != 'ocs-contracts/1':
            continue
        source = str(path.relative_to(ROOT))
        definition_hashes[source] = digest(path)
        definitions.extend(dict(c, definition_source=source) for c in document['contracts'])
    contracts = contract_review(inventory['contracts'], definitions)
    rows = []
    for name, program in sorted(programs.items()):
        path = (ROOT / name).resolve()
        if ROOT not in path.parents:
            raise ValueError(f'path outside repository: {name}')
        current_hash = digest(path) if path.is_file() else None
        check = checks.get(name)
        matching = check is not None and check['sha256'] == program['sha256']
        rows.append({
            'path': name,
            'category': program['category'],
            'inventory_sha256': program['sha256'],
            'current_sha256': current_hash,
            'source_state': ('missing' if current_hash is None else
                             'matches_inventory' if current_hash == program['sha256']
                             else 'changed_since_inventory'),
            'baseline_compile_evidence': ('absent' if check is None else
                                          'matching_source' if matching else 'different_source'),
            'baseline_expectation': check.get('expectation') if matching else None,
            'baseline_runtime': check.get('runtime', 'UNKNOWN') if matching else 'UNKNOWN',
            'case_ids': program['case_ids'],
            'scenario_review': ('review_case_contract_and_environment' if program['case_ids']
                                else 'needs_explicit_runtime_scenario_or_nonexecution_reason'),
            'fixture_hints': program['fixture_hints'],
        })
    report = {
        'scope': 'static evidence join; no programs executed; not semantic acceptance',
        'input_sha256': {'inventory': digest(inventory_path), 'source_checks': digest(baseline_path),
                         'contract_definitions': definition_hashes},
        'summary': {
            key: dict(sorted(collections.Counter(row[key] for row in rows).items()))
            for key in ('source_state', 'baseline_compile_evidence', 'baseline_runtime', 'scenario_review')
        },
        'contracts_without_cases': sorted(c['id'] for c in inventory['contracts'] if not c['cases']),
        'contract_review_summary': dict(sorted(collections.Counter(c['review_action'] for c in contracts).items())),
        'mandatory_contracts_without_cases': [c['id'] for c in contracts if c['review_action'] == 'missing_case_mapping'],
        'contract_reviews': contracts,
        'unmapped_contract_families': dict(sorted(collections.Counter(
            c['family'] for c in inventory['contracts'] if not c['cases']).items())),
        'unmapped_language_contracts': [
            c for c in contracts
            if not c['cases'] and not c['family'].startswith('Standard library:')
        ],
        'baseline_paths_outside_inventory': sorted(set(checks) - set(programs)),
        'limitations': [
            'Inventory is a historical snapshot; newly added files are not discovered here.',
            'UNKNOWN means no matching evidence in source-checks, not absence of other reports.',
            'Fixture hints do not authorize execution or classify safety.',
            'Matching compilation evidence does not establish runtime correctness.',
        ],
        'programs': rows,
        'historical_runtime_evidence': runtime_evidence(),
    }
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
