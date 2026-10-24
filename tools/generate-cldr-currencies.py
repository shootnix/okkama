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

def literal(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'))
def mapping(v):return literal(v).replace('{','[',1)[:-1]+']' if v else '[:]'
text=['module locale/internal/currencies v1.0.0;','// Generated from SHA-checked SIX ISO 4217 2026-09-17 and CLDR 46.1.0.',
'import "compare" take Order;import "strings/parse" take ParsedInt;',
'object CldrCurrencies {has tags,rows is @string;',
'CldrCurrencies(){tags='+literal('|'.join(rows))+'.split("|");rows='+literal('|'.join(rows.values()))+'.split("|");}',
'digits(code is string) is string{let values is %string:string='+mapping(codes)+';if values.has(code){values[code];}else{"";}}',
'profile(tag is string,system is string) is int{let index=self.search(tag,0,tags.size());if index<0{return -1;}let names=rows[index*3].split(",");let ids=rows[index*3+1].split(",");for (0..names.size())->at{if names[at]==system{return ParsedInt(ids[at]).value();}}ParsedInt(rows[index*3+2]).value();}',
'fields(profile is int) is @string{']
for i,m in enumerate(profiles):text.append('if profile=='+str(i)+'{return '+literal(m['fields'])+'.split("|");}')
text+=['let empty is @string=[];empty;}','compact(profile is int) is %string:string{']
for i,m in enumerate(profiles):text.append('if profile=='+str(i)+'{return '+mapping(m['compact'])+';}')
text+=['[:];}','currency(tag is string,code is string) is @string{']
chunks=[];chunk=[];size=0
for tag,row in name_rows.items():
 width=len(literal(row).encode())+len(tag)+96
 if chunk and size+width>65536:chunks.append(chunk);chunk=[];size=0
 chunk.append((tag,row));size+=width
if chunk:chunks.append(chunk)
args.output.parent.mkdir(exist_ok=True,parents=True)
imports=[]
for index,chunk in enumerate(chunks):
 name='CurrencyNames'+str(index);module='locale/internal/currency_names_'+str(index)
 imports.append('import "'+module+'" take '+name+';')
 text.append('if tag.order('+literal(chunk[-1][0])+')!=Order.greater{return '+name+'().row(tag,code);}')
 lines=['module '+module+' v1.0.0;','// Generated bounded CLDR currency translation delta shard.', 'import "locale/internal/currency_rows" take CurrencyTranslationRows;', 'object '+name+' {'+name+'();row(tag is string,code is string) is @string{']
 for tag,row in chunk:lines.append('if tag=='+literal(tag)+'{return CurrencyTranslationRows('+literal(row)+').row(code);}')
 lines.append('let empty is @string=[];empty;}}')
 (args.output.parent/('currency_names_'+str(index)+'.ok')).write_text('\n'.join(lines)+'\n')
text[1:1]=imports
text+=['let empty is @string=[];empty;}','private:', 'search(tag is string,start is int,stop is int) is int{if start>=stop{return -1;}let middle=start+((stop-start)/2).int();let order=tag.order(tags[middle]);if order==Order.equal{return middle;}if order==Order.less{self.search(tag,start,middle);}else{self.search(tag,middle+1,stop);}}', '}', 'object CurrencySymbolLetters {CurrencySymbolLetters();membership(origin is string) is bool{origin.length()==1 && '+literal(letters)+'.split(origin).size()>1;}}']
helper_text='module locale/internal/currency_rows v1.0.0;\n// Generated immutable packed currency row search.\nimport "compare" take Order;\n'+'object CurrencyTranslationRows {\nhas fields is @string;CurrencyTranslationRows(origin is string){fields=origin.split("|");}\nrow(code is string) is @string{let index=self.search(code,0,(fields.size()/10).int());if index<0{let empty is @string=[];return empty;}let values is @string=(1..10).map()->offset{fields[index*10+offset];};values;}\nprivate:\nsearch(code is string,start is int,stop is int) is int{if start>=stop{return -1;}let middle=start+((stop-start)/2).int();let order=code.order(fields[middle*10]);if order==Order.equal{return middle;}if order==Order.less{self.search(code,start,middle);}else{self.search(code,middle+1,stop);}}\n}'+'\n'
(args.output.parent/'currency_rows.ok').write_text(helper_text)
args.output.parent.mkdir(exist_ok=True,parents=True);args.output.write_text('\n'.join(text)+'\n')
print(f'compiled {len(codes)} ISO codes, {len(rows)} locale profiles, {len(profiles)} unique profiles, {len(name_rows)} translation delta rows; {args.output.stat().st_size} main bytes, {len(chunks)} bounded name modules')
