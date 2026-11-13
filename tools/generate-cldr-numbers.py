#!/usr/bin/env python3
"""Compile reviewed CLDR number symbols/patterns without runtime pattern eval."""
import argparse
import hashlib
import json
from pathlib import Path

root=Path(__file__).resolve().parents[1]
cli=argparse.ArgumentParser(description=__doc__)
cli.add_argument('--source',type=Path,default=root/'tools/data/cldr-46.1')
cli.add_argument('--output',type=Path,default=root/'std/locale/internal/numbers.ok')
args=cli.parse_args()
manifest=json.loads((args.source/'manifest.json').read_text())
for name,digest in manifest['sha256'].items():
    if hashlib.sha256((args.source/name).read_bytes()).hexdigest()!=digest:
        raise ValueError('source hash mismatch: '+name)
data=json.loads((args.source/'numberFormats.json').read_text())
if data['release']!=manifest['release']:
    raise ValueError('number release mismatch')

from cldr_number_patterns import pattern

symbol_names=['decimal','group','plusSign','minusSign','percentSign','perMille','exponential','nan','infinity']
profiles={};models=[];rows={}
for tag,entry in sorted(data['main'].items()):
    systems=sorted(k.removeprefix('symbols-numberSystem-') for k in entry if k.startswith('symbols-numberSystem-'))
    ids=[]
    for system in systems:
        symbols=entry['symbols-numberSystem-'+system]
        decimal=entry['decimalFormats-numberSystem-'+system]
        science=entry['scientificFormats-numberSystem-'+system]['standard']
        percent=entry['percentFormats-numberSystem-'+system]['standard']
        fields=[symbols[name] for name in symbol_names]
        for value in [decimal['standard'],science,percent]:fields.extend(map(str,pattern(value)))
        if any('|' in value for value in fields):raise ValueError('unsafe profile delimiter')
        compacts=[]
        for style in ['short','long']:
            compact={}
            for key,value in sorted(decimal.get(style,{}).get('decimalFormat',{}).items()):
                magnitude,category=key.split('-count-')
                if magnitude!='1'+'0'*(len(magnitude)-1) or category not in ['0','1','zero','one','two','few','many','other']:
                    raise ValueError('unreviewed compact key: '+key)
                # A single 0 means ordinary formatting, not a scaled compact unit.
                compact[str(len(magnitude)-1)+':'+category]=('|'.join(map(str,pattern(value))) if value!='0' else '')
            for key, value in compact.items():
                if value and not key.endswith(':other'):
                    other=compact.get(key.split(':')[0]+':other')
                    if other and value.split('|')[-1] not in ['0',other.split('|')[-1]]:
                        raise ValueError('category changes compact divisor: '+tag+' '+key)
            compacts.append(compact)
        model={'fields':'|'.join(fields),'short':compacts[0],'long':compacts[1]}
        identity=json.dumps(model,sort_keys=True,ensure_ascii=False)
        if identity not in profiles:profiles[identity]=len(models);models.append(model)
        ids.append(profiles[identity])
    other=entry['otherNumberingSystems'];row=[entry['defaultNumberingSystem'],other.get('native',''),other.get('traditional',''),other.get('finance',''),str(entry['minimumGroupingDigits']),','.join(systems),','.join(map(str,ids))]
    rows[tag]='|'.join(row)

numbering=json.loads((args.source/'numberingSystems.json').read_text())['supplemental']
if numbering['version']!={'_unicodeVersion':'16.0.0','_cldrVersion':'46'}:raise ValueError('numbering version mismatch')
digits={name:entry['_digits'] for name,entry in numbering['numberingSystems'].items() if entry['_type']=='numeric'}
if any(len(value)!=10 or len(set(value))!=10 for value in digits.values()):raise ValueError('unreviewed digits')

from cldr_data import Tables

# The tables are runtime data (#223); the module only names them.
tables=Tables('numbers')
for tag,value in sorted(rows.items()):tables.add('row',tag,value.split('|'))
for i,model in enumerate(models):
    tables.add('profile',i,model['fields'].split('|'))
    for style in ['short','long']:tables.add(style,i,[part for key,value in model[style].items() for part in (key,value)])
for system,value in sorted(digits.items()):tables.add('digits',system,[value])
data,records,size=tables.save()
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text('''module locale/internal/numbers v1.0.0;

// Generated from SHA-checked CLDR 46.1.0 / Unicode 16.0.0 by
// tools/generate-cldr-numbers.py. The tables are runtime data
// (cldr-numbers.dat, #223).
import "runtime/cldr" take cldr_fields;

// CLDR number symbols, patterns and numbering systems by locale.
object CldrNumbers {
    // The data.
    CldrNumbers();

    // Default, native, traditional and finance numbering systems, minimum
    // grouping digits, the locale's systems and their profiles; empty for an
    // unknown tag.
    row(tag is string) is @string {
        cldr_fields("numbers.row", tag);
    }

    // Symbols, then the decimal, scientific and percent patterns.
    profile(id is int) is @string {
        cldr_fields("numbers.profile", id.string());
    }

    // Compact patterns by "exponent:category".
    compact(id is int, long is bool) is %string:string {
        let table = if long {
            "numbers.long";
        }
        else {
            "numbers.short";
        };
        let pairs = cldr_fields(table, id.string());
        let empty is %string:string = [:];
        (0..(pairs.size() / 2).int()).reduce(empty) -> kept, index {
            kept + [pairs[index * 2]: pairs[index * 2 + 1]];
        };
    }

    // The ten digits of a numeric system; empty for an unknown one.
    digits(system is string) is string {
        let found = cldr_fields("numbers.digits", system);
        if found.size() > 0 {
            found[0];
        }
        else {
            "";
        };
    }
}
''')
print(f'compiled {len(rows)} locales, {len(models)} number profiles, {len(digits)} numeric systems; {records} records, {size} bytes in {data.name}')
