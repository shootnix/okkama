#!/usr/bin/env python3
"""Keep production dependency graphs of Sparrow and the IR runtime separate."""
from pathlib import Path
import subprocess

manifest = Path(__file__).resolve().parents[1] / 'Cargo.toml'
for package, forbidden in (
    # The core bridges natives to okkama-runtime-core, never to an interpreter.
    ('sparrow-core', {'okkama-compiler', 'okkama-runtime',
                      'sparrow-vm', 'sparrow-runtime', 'sparrow-cli', 'corosensei'}),
    ('sparrow-cli', {'okkama-compiler', 'okkama-runtime'}),
    ('okkama-runtime', {'sparrow-vm', 'sparrow-runtime', 'sparrow-cli'}),
):
    result = subprocess.run(
        ['cargo', 'tree', '--offline', '--manifest-path', str(manifest),
         '-p', package, '--edges', 'normal,build', '--prefix', 'none', '--format', '{p}'],
        check=True, text=True, capture_output=True,
    )
    packages = {line.split()[0] for line in result.stdout.splitlines() if line.strip()}
    unexpected = packages & forbidden
    if unexpected:
        raise SystemExit(f'{package}: forbidden production dependencies: {sorted(unexpected)}')
    print(f'{package}: independent production dependency graph OK')
