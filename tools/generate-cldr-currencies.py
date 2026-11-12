#!/usr/bin/env python3
"""Compile SHA-checked ISO currency coordinates and inherited CLDR representations."""
import argparse,hashlib,json,xml.etree.ElementTree as ET
from pathlib import Path
from cldr_number_patterns import pattern
root=Path(__file__).resolve().parents[1]
cli=argparse.ArgumentParser(description=__doc__);cli.add_argument('--output',type=Path,default=root/'std/locale/internal/currencies.ok');args=cli.parse_args()
source=root/'tools/data/cldr-46.1';manifest=json.loads((source/'manifest.json').read_text())
for name,digest in manifest['sha256'].items():
 if hashlib.sha256((source/name).read_bytes()).hexdigest()!=digest:raise ValueError('CLDR hash mismatch: '+name)
iso_folder=root/'tools/data/iso-4217-2026-09-17';iso_manifest=json.loads((iso_folder/'manifest.json').read_text());raw=(iso_folder/'list-one.xml').read_bytes()
if hashlib.sha256(raw).hexdigest()!=iso_manifest['sha256']:raise ValueError('ISO hash mismatch')
iso=ET.fromstring(raw)
if iso.attrib['Pblshd']!=iso_manifest['published']:raise ValueError('ISO date mismatch')
codes={}
for row in iso.findall('CcyTbl/CcyNtry'):
 code=row.findtext('Ccy');digits=row.findtext('CcyMnrUnts')
 if not code:continue
 if len(code)!=3 or not code.isascii() or not code.isalpha() or code!=code.upper() or digits not in ('0','2','3','4','N.A.'):raise ValueError('unreviewed ISO entry')
 if code in codes and codes[code]!=digits:raise ValueError('conflicting ISO minor units')
 codes[code]=digits
source_data=json.loads((source/'currencies.json').read_text())
if source_data['release']!=manifest['release']:raise ValueError('currency release mismatch')
data=source_data['main'];parents=json.loads((source/'parentLocales.json').read_text())['supplemental']['parentLocales']['parentLocale'];numbers=json.loads((source/'numberFormats.json').read_text())['main']
def parent(tag):return parents.get(tag,tag.rsplit('-',1)[0] if '-' in tag else 'root')
def merge(a,b):
 result=dict(a)
 for k,v in b.items():result[k]=merge(a.get(k,{}),v) if isinstance(v,dict) else v
 return result
cache={}
def effective(tag):
 if tag in cache:return cache[tag]
 value=data.get(tag,{})
 result=value if tag=='root' else merge(effective(parent(tag)),value)
 cache[tag]=result;return result
categories=json.loads((source/'currencySymbolCategories.json').read_text())
if categories['unicode']!=manifest['unicode']:raise ValueError('symbol Unicode version mismatch')
for row in data.values():
 for entry in row.get('currencies',{}).values():
  for key,value in entry.items():
   if key.startswith('symbol') and value:
    for char in (value[0],value[-1]):
     if char not in categories['categories']:raise ValueError('unreviewed symbol boundary '+repr(char))
letters=''.join(char for char,category in sorted(categories['categories'].items()) if category.startswith('L'))
fields=['displayName']+['displayName-count-'+c for c in ['zero','one','two','few','many','other']]+['symbol','symbol-alt-narrow']
name_rows={}
for tag,row in sorted(data.items()):
 values=[]
 for code,entry in sorted(row.get('currencies',{}).items()):
  values.extend([code]+[entry.get(k,'') for k in fields])
 if any('|' in s or '\n' in s for s in values):raise ValueError('unsafe currency record delimiter')
 if values:name_rows[tag]='|'.join(values)
profiles=[];identities={};rows={}
for tag,entry in sorted(numbers.items()):
 resolved=effective(tag);formats=resolved['formats'];systems=sorted(formats);ids=[]
 for system in systems:
  fmt=formats[system];standard=fmt['standard'];accounting=fmt.get('accounting',standard)
  values=[]
  for origin in [standard,accounting,fmt.get('standard-alphaNextToNumber',standard),fmt.get('accounting-alphaNextToNumber',accounting)]:
   values.extend(map(str,pattern(origin)))
  for category in ['zero','one','two','few','many','other']:values.append(fmt.get('unitPattern-count-'+category,''))
  monetary=resolved.get('monetarySymbols',{}).get(system,{})
  values.extend([monetary.get('currencyDecimal',''),monetary.get('currencyGroup','')])
  if any('|' in value for value in values):raise ValueError('unsafe currency profile delimiter')
  compact={}
  for key,origin in sorted(fmt.get('short',{}).get('standard',{}).items()):
   power,category=key.split('-count-');category,_,alt=category.partition('-alt-')
   if power!='1'+'0'*(len(power)-1) or category not in ['0','1','zero','one','two','few','many','other'] or alt not in ['', 'alphaNextToNumber']:raise ValueError('unreviewed compact currency key '+key)
   compact[str(len(power)-1)+':'+category+(':alpha' if alt else '')]='|'.join(map(str,pattern(origin))) if origin!='0' else ''
  model={'fields':'|'.join(values),'compact':compact};identity=json.dumps(model,ensure_ascii=False,sort_keys=True)
  if identity not in identities:identities[identity]=len(profiles);profiles.append(model)
  ids.append(identities[identity])
  for v in values[:32]:
   if '¤¤' in v:raise ValueError('unreviewed currency marker run')
 default=entry['defaultNumberingSystem'];default=default if default in systems else 'latn'
 rows[tag]='|'.join([','.join(systems),','.join(map(str,ids)),str(ids[systems.index(default)])])

from cldr_data import Tables

# The tables are runtime data (#223); the module only names them.
tables=Tables('currencies')
for code,value in sorted(codes.items()):tables.add('digits',code,[value])
for tag,row in rows.items():tables.add('row',tag,row.split('|'))
for i,m in enumerate(profiles):
 tables.add('fields',i,m['fields'].split('|'))
 tables.add('compact',i,[part for key,value in m['compact'].items() for part in (key,value)])
for tag,row in name_rows.items():
 values=row.split('|')
 for at in range(0,len(values),10):tables.add('name',tag+'|'+values[at],values[at+1:at+10])
for char in letters:tables.add('letter',char,['1'])
data,records,size=tables.save()
for stale in args.output.parent.glob('currency_names_*.ok'):stale.unlink()
(args.output.parent/'currency_rows.ok').unlink(missing_ok=True)
args.output.parent.mkdir(exist_ok=True,parents=True)
args.output.write_text('''module locale/internal/currencies v1.0.0;

// Generated from SHA-checked SIX ISO 4217 2026-09-17 and CLDR 46.1.0 by
// tools/generate-cldr-currencies.py. The tables are runtime data
// (cldr-currencies.dat, #223).
import "runtime/cldr" take cldr_fields;
import "strings/parse" take Number;

object CldrCurrencies {
    CldrCurrencies();

    // ISO 4217 minor units ("0", "2", "3", "4" or "N.A."); empty for an
    // unknown code.
    digits(code is string) is string {
        let found = cldr_fields("currencies.digits", code);
        if found.size() > 0 {
            found[0];
        }
        else {
            "";
        };
    }

    // The currency profile of a locale tag and numbering system: the
    // system's own, else the locale's default; -1 for an unknown tag.
    profile(tag is string, system is string) is int {
        let row = cldr_fields("currencies.row", tag);
        if row.size() == 0 {
            return -1;
        }
        let names = row[0].split(",");
        let ids = row[1].split(",");
        for (0..names.size()) -> at {
            if names[at] == system {
                return Number(ids[at]).int();
            }
        }
        Number(row[2]).int();
    }

    // Standard, accounting and alphabetic-boundary patterns, unit patterns
    // by plural category, and the currency decimal and group symbols.
    fields(profile is int) is @string {
        cldr_fields("currencies.fields", profile.string());
    }

    // Compact patterns by "exponent:category" and "exponent:category:alpha".
    compact(profile is int) is %string:string {
        let pairs = cldr_fields("currencies.compact", profile.string());
        let empty is %string:string = [:];
        (0..(pairs.size() / 2).int()).reduce(empty) -> kept, index {
            kept + [pairs[index * 2]: pairs[index * 2 + 1]];
        };
    }

    // Display name, the names by plural category, symbol and narrow symbol
    // that a locale tag itself gives a currency; empty when it gives none.
    currency(tag is string, code is string) is @string {
        cldr_fields("currencies.name", tag + "|" + code);
    }
}

object CurrencySymbolLetters {
    CurrencySymbolLetters();

    // Is a one-character text a letter at a currency symbol's boundary?
    membership(origin is string) is bool {
        origin.length() == 1 && cldr_fields("currencies.letter", origin).size() > 0;
    }
}
''')
print(f'compiled {len(codes)} ISO codes, {len(rows)} locale profiles, {len(profiles)} unique profiles, {len(name_rows)} translation delta rows; {records} records, {size} bytes in {data.name}')
