#!/usr/bin/env python3
"""Reuse cookbook protocol/data oracles with an explicit Sparrow binary."""
import argparse
import json
import os
from pathlib import Path
import runpy

ROOT = Path(__file__).resolve().parents[3]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--binary', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
os.environ['SPARROW_BASELINE_CLI'] = str(a.binary.resolve())
os.environ['OKKAMA'] = str(Path(__file__).with_name('baseline-run'))
os.environ['OKKAMA_TRACE'] = 'off'
module = runpy.run_path(str(ROOT / 'examples/cookbook/script/verify.py'), run_name='baseline')
error = None
try:
    module['main']()
except Exception as exception:
    error = str(exception)
finally:
    results = module['RESULTS']
    a.out.parent.mkdir(parents=True, exist_ok=True)
    a.out.write_text(json.dumps(dict(binary=str(a.binary.resolve()), results=results,
                                    error=error, completed=error is None),
                               ensure_ascii=False, indent=2) + '\n')
if error:
    raise SystemExit(error)
