#!/usr/bin/env python3
"""Keep Sparrow independent of the compiler and removed interpreter."""
from pathlib import Path
import subprocess

manifest = Path(__file__).resolve().parents[1] / 'Cargo.toml'
for package, forbidden in (
    # The core bridges natives to okkama-runtime-core, never to an interpreter.
    # Only native promises remain in the shared delivery scheduler.
    ('sparrow-core', {'okkama-compiler', 'okkama-runtime', 'sparrow-cli'}),
    ('sparrow-cli', {'okkama-compiler', 'okkama-runtime'}),
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
