"""CLDR tables as runtime data (#223).

The CLDR generators write their tables to gzip text files next to the other
runtime data (okkama-rs/crates/okkama-compiler/runtime/jvm/src/okkama/rt).
The runtimes read a file the first time one of its tables is asked for, and
std/runtime/cldr.ok answers lookups with `cldr_fields(table, key)`.

One record is one line: the table, the key and the fields, separated by tabs.
The table name starts with the file's family and a dot (`numbers.row` lives
in `cldr-numbers.dat`). A record with no fields is not written: a missing key
and an empty record both read as no fields.
"""
import gzip
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / 'okkama-rs/crates/okkama-compiler/runtime/jvm/src/okkama/rt'


class Tables:
    """Records of one family, written by `save` to `cldr-<family>.dat`."""

    def __init__(self, family):
        self.family = family
        self.records = {}

    def add(self, table, key, fields):
        name = self.family + '.' + table
        fields = [str(field) for field in fields]
        for part in [name, str(key)] + fields:
            if '\t' in part or '\n' in part or '\r' in part:
                raise ValueError('unsafe CLDR record delimiter: ' + repr(part))
        if (name, str(key)) in self.records:
            raise ValueError('duplicate CLDR record: ' + name + ' ' + str(key))
        if fields:
            self.records[(name, str(key))] = fields

    def save(self, directory=RUNTIME):
        lines = ['\t'.join([name, key] + fields) for (name, key), fields in sorted(self.records.items())]
        data = ('\n'.join(lines) + '\n').encode('utf-8')
        path = Path(directory) / ('cldr-' + self.family + '.dat')
        # mtime 0 keeps the file byte-identical across runs.
        path.write_bytes(gzip.compress(data, compresslevel=9, mtime=0))
        return path, len(lines), len(data)
