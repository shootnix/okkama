#!/usr/bin/env python3
"""Compile the pinned IANA preferred-value and grandfathered tag snapshot."""
import argparse
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1]
cli = argparse.ArgumentParser(description=__doc__)
cli.add_argument('--source',type=Path,default=root/'tools/data/iana-language-2026-09-17')
cli.add_argument('--output',type=Path,default=root/'std/locale/internal/iana.ok')
args = cli.parse_args()
manifest = json.loads((args.source/'manifest.json').read_text())
raw = (args.source/'language-subtag-registry.txt').read_bytes()
if hashlib.sha256(raw).hexdigest() != manifest['sha256']:
    raise ValueError('IANA registry hash mismatch')
chunks = raw.decode('utf-8').split('%%')
if chunks[0].strip() != 'File-Date: '+manifest['file_date']:
    raise ValueError('IANA file-date mismatch')
preferred,grandfathered,extlangs = {},{},{}
for chunk in chunks[1:]:
    fields = {}
    last = None
    for line in chunk.strip().splitlines():
        if line.startswith(' '):
            fields[last][-1] += ' '+line.strip()
        else:
            key,value = line.split(': ',1)
            fields.setdefault(key,[]).append(value)
            last = key
    kind = fields['Type'][0]
    subtag = fields.get('Subtag',fields.get('Tag'))[0].lower()
    target = fields.get('Preferred-Value',[''])[0]
    if kind=='grandfathered':
        grandfathered[subtag]=target
    if target:
        preferred[kind+'|'+subtag]=target
    if kind=='extlang':
        prefixes=fields.get('Prefix',[])
        if len(prefixes)!=1 or target.lower()!=subtag:
            raise ValueError('unreviewed extlang record: '+subtag)
        extlangs[subtag]=prefixes[0].lower()
def literal(data):
    if not data:
        return '[:]'
    return json.dumps(dict(sorted(data.items())),separators=(',',':')).replace('{','[',1)[:-1]+']'
# Collapse preferred subtag chains at build time, retaining the last preferred
# spelling. A future source cycle is a generation error, never runtime truncation.
resolved = {}
for key,value in preferred.items():
    kind,_ = key.split('|',1)
    seen = {key}
    target = value
    while kind+'|'+target.lower() in preferred:
        following = kind+'|'+target.lower()
        if preferred[following].lower() == target.lower():
            break
        if following in seen:
            raise ValueError('preferred-value cycle: '+key)
        seen.add(following)
        target = preferred[following]
    resolved[key] = target
by_kind = {}
for key,value in resolved.items():
    kind,tag = key.split('|',1)
    by_kind.setdefault(kind,{})[tag] = value
whole_tags = dict(grandfathered)
whole_tags.update(by_kind.get('redundant',{}))
alias_cases = ''.join('case '+json.dumps(kind)+'{'+literal(values)+';}'
                      for kind,values in sorted(by_kind.items()))
text='''module locale/internal/iana v1.0.0;
// Generated from SHA-checked IANA Language Subtag Registry, file-date DATE.
// Materialize only the queried registry group, not every table for every tag.
object IanaLanguages {
 IanaLanguages();
 version() is string{"DATE";}
 alias(kind is string,tag is string) is string{
  let values is %string:string=match kind{ALIASES else{[:];}};
  if values.has(tag){values[tag];}else{tag;}
 }
 grandfathered(tag is string) is bool{
  if tag.split("-").size()==1{return false;}
  let values is %string:string=GRANDFATHERED;values.has(tag);
 }
 preferred_tag(tag is string) is string{
  if tag.split("-").size()==1{return "";}
  let values is %string:string=WHOLE;
  if values.has(tag){values[tag];}else{"";}
 }
 extlang_prefix(tag is string) is string{let values is %string:string=EXTLANGS;if values.has(tag){values[tag];}else{"";}}
}
object IanaLanguageDataVersion {IanaLanguageDataVersion();string() is string{"DATE";}}
'''
for key,value in [('DATE',manifest['file_date']),('ALIASES',alias_cases),('GRANDFATHERED',literal(grandfathered)),('WHOLE',literal(whole_tags)),('EXTLANGS',literal(extlangs))]:
    text=text.replace(key,value)
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text(text)
print(f'compiled {len(preferred)} preferred values, {len(grandfathered)} grandfathered tags, {len(extlangs)} extlangs')
