#!/usr/bin/env python3
"""CLI for the OIR-2 ownership reference model."""
import argparse
import json
from pathlib import Path
import sys

from checker import run, verify
from ir import ModelError, parse


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('run', 'verify', 'format'))
    parser.add_argument('file', type=Path)
    parser.add_argument('--events', action='store_true', help='print deterministic JSON event stream')
    args = parser.parse_args()
    try:
        program = parse(args.file.read_text())
        if args.command == 'format':
            print(program.text(), end='')
        elif args.command == 'verify':
            states = verify(program)
            print(f'OK: {len(states)} verified path(s)')
        else:
            verify(program)
            state = run(program)
            if args.events:
                for event in state.events:
                    print(json.dumps(event, ensure_ascii=False, sort_keys=True))
            print(json.dumps({'destroy': state.destroy_trace, 'log': state.log,
                              'checkpoints': state.checkpoints, 'metrics': dict(state.metrics)},
                             ensure_ascii=False, sort_keys=True))
    except ModelError as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
