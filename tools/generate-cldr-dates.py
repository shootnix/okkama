#!/usr/bin/env python3
"""Compile pinned Gregorian standard styles and explicit timezone-name data."""
import argparse,datetime,hashlib,json
from pathlib import Path
root=Path(__file__).resolve().parents[1];folder=root/'tools/data/cldr-46.1'
cli=argparse.ArgumentParser(description=__doc__);cli.add_argument('--output-directory',type=Path,default=root/'std/locale/internal');args=cli.parse_args();args.output_directory.mkdir(parents=True,exist_ok=True)
manifest=json.loads((folder/'manifest.json').read_text())
for name,digest in manifest['sha256'].items():
 if hashlib.sha256((folder/name).read_bytes()).hexdigest()!=digest:raise ValueError('source hash mismatch '+name)
parents=json.loads((folder/'parentLocales.json').read_text())['supplemental']['parentLocales']['parentLocale']
def parent(tag):return parents.get(tag,tag.rsplit('-',1)[0] if '-' in tag else 'root')
def merge(a,b):
 r=dict(a) if isinstance(a,dict) else {}
 for k,v in b.items():r[k]=merge(a.get(k,{}),v) if isinstance(v,dict) else v
 return r
def resolve(rows):
 cache={}
 def value(tag):
  if tag not in cache:cache[tag]=rows.get(tag,{}) if tag=='root' else merge(value(parent(tag)),rows.get(tag,{}))
  return cache[tag]
 return value
literal=lambda x:json.dumps(x,ensure_ascii=False,separators=(',',':'))
def packed(values):
 if any('|' in v for v in values):raise ValueError('unsafe packed delimiter')
 return '|'.join(values)
def mapping(v):return literal(v).replace('{','[',1)[:-1]+']' if v else '[:]'
def write(name,lines):
 path=args.output_directory/(name+'.ok');path.write_text('\n'.join(lines)+'\n');return path

from cldr_data import Tables
# The tables are runtime data (#223); the modules only name them.
dates=Tables('dates');zones_data=Tables('zones')
MODULE_GREGORIAN='''module locale/internal/gregorian v1.0.0;

// Generated from SHA-checked CLDR 46.1.0 Gregorian standard styles by
// tools/generate-cldr-dates.py. The tables are runtime data
// (cldr-dates.dat, #223).
import "runtime/cldr" take cldr_fields;
import "strings/parse" take Number;

object CldrGregorian {
    CldrGregorian();

    // The profile of a locale tag; -1 for an unknown tag.
    profile(tag is string) is int {
        let found = cldr_fields("dates.profile", tag);
        if found.size() > 0 {
            Number(found[0]).int();
        }
        else {
            -1;
        };
    }

    // Month, weekday, era and day-period names, date and time programs, and
    // date-time combinations.
    fields(id is int) is @string {
        cldr_fields("dates.fields", id.string());
    }

    // A compiled date or time pattern; empty for an unknown id.
    program(id is int) is string {
        let found = cldr_fields("dates.program", id.string());
        if found.size() > 0 {
            found[0];
        }
        else {
            "";
        };
    }
}
'''
MODULE_DAY_PERIODS='''module locale/internal/day_periods v1.0.0;

// Generated CLDR formatting-rule boundaries, not selection-rule labels, by
// tools/generate-cldr-dates.py. The table is runtime data (cldr-dates.dat,
// #223).
import "runtime/cldr" take cldr_fields;

object CldrDayPeriods {
    CldrDayPeriods();

    // Period, start and end tick, and exactness per rule; empty for an
    // unknown tag.
    rules(tag is string) is @string {
        cldr_fields("dates.periods", tag);
    }
}
'''
MODULE_ZONE_META='''module locale/internal/zone_meta v1.0.0;

// Generated historical CLDR UTC boundaries, without a host timezone lookup,
// by tools/generate-cldr-dates.py. The tables are runtime data
// (cldr-zones.dat, #223).
import "runtime/cldr" take cldr_fields;

object CldrZoneMeta {
    CldrZoneMeta();

    // The CLDR key of a zone name or alias; the name itself when unknown.
    canonical(name is string) is string {
        let found = cldr_fields("zones.canonical", name);
        if found.size() > 0 {
            found[0];
        }
        else {
            name;
        };
    }

    // Start, end and metazone per interval; empty for an unknown zone.
    intervals(name is string) is @string {
        cldr_fields("zones.intervals", name);
    }
}
'''
MODULE_ZONE_NAMES='''module locale/internal/zone_names v1.0.0;

// Generated sparse localized z/zzzz names and GMT patterns by
// tools/generate-cldr-dates.py. The tables are runtime data
// (cldr-zones.dat, #223).
import "runtime/cldr" take cldr_fields;

object CldrZoneNames {
    CldrZoneNames();

    // Hour, GMT and GMT-zero formats; empty for a tag without them.
    gmt(tag is string) is @string {
        cldr_fields("zones.gmt", tag);
    }

    // Short standard, short daylight, long standard and long daylight names
    // of "m:<metazone>" or "z:<zone>"; empty when the tag has none.
    row(tag is string, key is string) is @string {
        cldr_fields("zones.name", tag + "|" + key);
    }
}
'''
def tokens(origin):
 override=''
 if isinstance(origin,dict):
  if set(origin)!={'_value','_numbers'} or origin['_numbers']!='M=romanlow':raise ValueError('unreviewed field numbering '+str(origin))
  override='M';origin=origin['_value']
 records=[];quoted=False;i=0;text=''
 def flush():
  nonlocal text
  if text:records.append(['literal','0',text]);text=''
 while i<len(origin):
  c=origin[i]
  if c=="'":
   if i+1<len(origin) and origin[i+1]=="'":text+="'";i+=2;continue
   quoted=not quoted;i+=1;continue
  if not quoted and c.isascii() and c.isalpha():
   flush();end=i+1
   while end<len(origin) and origin[end]==c:end+=1
   count=end-i
   if c not in 'GyMLdEchHmsaBz' or count<1 or count>6:raise ValueError('unreviewed field '+origin[i:end])
   if c=='c' and count<3:raise ValueError('numeric localized weekday needs week-data contract')
   if c=='z' and count>4:raise ValueError('unreviewed timezone width')
   records.append(['roman-month' if c==override else c,str(count),'']);i=end
  else:text+=c;i+=1
 if quoted:raise ValueError('unbalanced pattern quoting')
 flush()
 if len(records)>128:raise ValueError('pattern token budget')
 return packed([v for row in records for v in row])
def combination(origin):
 result='';quoted=False;i=0
 while i<len(origin):
  char=origin[i]
  if char=="'":
   if i+1<len(origin) and origin[i+1]=="'":result+="'";i+=2;continue
   quoted=not quoted
  else:result+=char
  i+=1
 if quoted:raise ValueError('combination quote imbalance')
 return result
raw=json.loads((folder/'gregorian.json').read_text());greg=resolve(raw['main']);all_tags=sorted(json.loads((folder/'numberFormats.json').read_text())['main'])
if raw['release']!=manifest['release']:raise ValueError('Gregorian release mismatch')
programs=[];program_ids={};profiles=[];profile_ids={};rows={}
def program(p):
 code=tokens(p)
 if code not in program_ids:program_ids[code]=len(programs);programs.append(code)
 return str(program_ids[code])
periods=['am','pm','midnight','noon','morning1','morning2','afternoon1','afternoon2','evening1','evening2','night1','night2']
styles=['full','long','medium','short'];weekdays=['mon','tue','wed','thu','fri','sat','sun']
for tag in all_tags:
 g=greg(tag);fields=[]
 for context in ['format','stand-alone']:
  for width in ['abbreviated','wide','narrow']:
   group=g['months'][context][width];fields.extend(group[str(n)] for n in range(1,13))
 for context in ['format','stand-alone']:
  for width in ['abbreviated','wide','narrow','short']:
   group=g['days'][context].get(width,g['days'][context]['abbreviated']);fields.extend(group[n] for n in weekdays)
 fields.extend(g['eras'][k]['1'] for k in ['eraAbbr','eraNames','eraNarrow'])
 for width in ['abbreviated','wide','narrow']:
  group=g['dayPeriods']['format'][width];fields.extend(group.get(n,'') for n in periods)
 if len(fields)!=167:raise ValueError('Gregorian symbol shape')
 for family in ['dateFormats','timeFormats']:fields.extend(program(g[family][s]) for s in styles)
 for s in styles:
  p=g['dateTimeFormats'][s]
  if not isinstance(p,str) or p.count('{0}')!=1 or p.count('{1}')!=1:raise ValueError('unreviewed date/time combination')
  fields.append(combination(p))
 model=packed(fields)
 if model not in profile_ids:profile_ids[model]=len(profiles);profiles.append(model)
 rows[tag]=profile_ids[model]
for tag,profile in rows.items():dates.add('profile',tag,[profile])
for i,p in enumerate(profiles):dates.add('fields',i,p.split('|'))
for i,p in enumerate(programs):dates.add('program',i,[p])
lines=MODULE_GREGORIAN.splitlines()
write('gregorian',lines)
# Day-period boundaries are nanosecond ticks in a civil day. Exact points precede
# intervals; wrapped intervals retain their two endpoint comparisons at runtime.
day=json.loads((folder/'dayPeriods.json').read_text())['supplemental']
if day['version']['_cldrVersion']!='46':raise ValueError('period version mismatch')
def tick(t):
 h,m=map(int,t.split(':'));return (h*3600+m*60)*1000000000
for tag,rules in sorted(day['dayPeriodRuleSet'].items()):
 records=[]
 for exact in [True,False]:
  for name,rule in sorted(rules.items()):
   if ('_at' in rule)!=exact:continue
   if name not in periods:raise ValueError('unreviewed period '+name)
   if exact:
    if set(rule)!={'_at'}:raise ValueError('unreviewed exact rule')
    start=tick(rule['_at']);end=start
   else:
    if set(rule)!={'_from','_before'}:raise ValueError('unreviewed interval rule')
    start=tick(rule['_from']);end=tick(rule['_before'])
   records.extend([str(periods.index(name)),str(start),str(end),'1' if exact else '0'])
  if len(records)>64*4:raise ValueError('day-rule budget')
 if not records:raise ValueError('empty day-period rules '+tag)
 dates.add('periods',tag,packed(records).split('|'))
write('day_periods',MODULE_DAY_PERIODS.splitlines())
# Explicit IANA/legacy name aliases and historical UTC metazone intervals.
meta=json.loads((folder/'metaZones.json').read_text())['supplemental'];aliases=json.loads((folder/'timeZoneAliases.json').read_text())['keyword']['u']['tz'];zones={}
def walk(tree,prefix=''):
 for key,v in tree.items():
  name=prefix+key
  if isinstance(v,list):zones[name]=v
  else:walk(v,name+'/')
walk(meta['metaZones']['metazoneInfo']['timezone'])
if meta['version']!={'_unicodeVersion':'16.0.0','_cldrVersion':'46'}:raise ValueError('metazone version mismatch')
canonical={}
for row in aliases.values():
 if not isinstance(row,dict) or '_alias' not in row:continue
 names=row['_alias'].split();target=next((n for n in names if n in zones),names[0])
 for name in names:canonical[name]=target
# CLDR zone-specific names use the same preferred key even without a metazone.
for key in ['Etc/UTC','Etc/GMT']:canonical[key]=key
for name,target in sorted(canonical.items()):zones_data.add('canonical',name,[target])
epoch=datetime.datetime(1970,1,1)
def unix(t):return int((datetime.datetime.fromisoformat(t)-epoch).total_seconds())
for name,entries in sorted(zones.items()):
 records=[];previous=None
 for entry in entries:
  r=entry['usesMetazone'];start=unix(r['_from']) if '_from' in r else -9223372036854775808;end=unix(r['_to']) if '_to' in r else 9223372036854775807
  if start>=end or (previous is not None and start<previous):raise ValueError('overlapping metazone interval')
  previous=end;records.extend([str(start),str(end),r['_mzone']])
 if not records:raise ValueError('empty metazone intervals '+name)
 zones_data.add('intervals',name,packed(records).split('|'))
write('zone_meta',MODULE_ZONE_META.splitlines())
# Sparse timezone-name fields are consulted individually through LocaleFallbacks.
zraw=json.loads((folder/'timeZoneNames.json').read_text());name_rows={};gmt_rows={}
for tag,row in sorted(zraw['main'].items()):
 gmt_rows[tag]=packed([row.get(k,'') for k in ['hourFormat','gmtFormat','gmtZeroFormat']]);entries={}
 def name_tree(tree,prefix,kind):
  if 'long' in tree or 'short' in tree:
   entries[kind+prefix]=[tree.get(w,{}).get(t,'') for w,t in [('short','standard'),('short','daylight'),('long','standard'),('long','daylight')]]
  else:
   for key,v in tree.items():name_tree(v,prefix+('/' if prefix else '')+key,kind)
 for name,value in row.get('metazone',{}).items():name_tree(value,name,'m:')
 name_tree(row.get('zone',{}),'','z:')
 values=[]
 for name,fields in sorted(entries.items()):values.extend([name]+fields)
 if values:name_rows[tag]=packed(values)
for tag,row in gmt_rows.items():zones_data.add('gmt',tag,row.split('|'))
for tag,row in name_rows.items():
 values=row.split('|')
 for at in range(0,len(values),5):zones_data.add('name',tag+'|'+values[at],values[at+1:at+5])
write('zone_names',MODULE_ZONE_NAMES.splitlines())
_,date_records,date_size=dates.save();_,zone_records,zone_size=zones_data.save()
print(f'compiled {len(rows)} Gregorian rows, {len(profiles)} profiles, {len(programs)} programs, {len(zones)} historical zone keys; {date_records} date records ({date_size} bytes), {zone_records} zone records ({zone_size} bytes)')
