"""Shared checked LDML numeric skeleton/affix compiler; no source I/O."""
def tokens(pattern):
    parts=[[]];quoted=False;index=0
    while index<len(pattern):
        char=pattern[index]
        if char=="'":
            if index+1<len(pattern) and pattern[index+1]=="'":
                parts[-1].append(("'",False));index+=2;continue
            quoted=not quoted;index+=1;continue
        if char==';' and not quoted:parts.append([])
        else:parts[-1].append((char,not quoted))
        index+=1
    if quoted or len(parts)>2:raise ValueError('unreviewed quoting/subpattern: '+pattern)
    return parts

def affix(parts):
    markers={'-':'{minus}','+':'{plus}','%':'{percent}','‰':'{permille}'}
    text=''.join(markers.get(char,char) if special else char for char,special in parts)
    if '|' in text:raise ValueError('unsafe pattern delimiter')
    return text

def subpattern(parts):
    start=next((i for i,(char,special) in enumerate(parts) if special and char in '#0'),len(parts))
    end=start
    while end<len(parts) and parts[end][1] and parts[end][0] in '#0.,E+':end+=1
    body=''.join(char for char,_ in parts[start:end])
    mantissa=body.split('E')[0];integer=mantissa.split('.')[0]
    groups=integer.split(',');primary=len(groups[-1]) if len(groups)>1 else 0
    secondary=len(groups[-2]) if len(groups)>2 else primary
    exp=body.split('E')[1].count('0') if 'E' in body else 0
    zeroes=integer.count('0')
    return [affix(parts[:start]),affix(parts[end:]),primary,secondary,exp,zeroes]

def pattern(origin):
    parts=tokens(origin);positive=subpattern(parts[0]);negative=subpattern(parts[1]) if len(parts)==2 else None
    if negative and negative[2:]!=positive[2:]:raise ValueError('different negative numeric body: '+origin)
    return positive[:2]+(negative[:2] if negative else ['{minus}'+positive[0],positive[1]])+positive[2:]
