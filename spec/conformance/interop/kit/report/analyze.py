import json,glob,os,re,hashlib,statistics as st,sys,collections
R='<round-2>'
IDS=['A-sonnet','B-sol','C-grok','D-gemini','E-kimi','F-glm']
LANG={'A-sonnet':'TypeScript/Node 22 (+WASM)','B-sol':'Python + C ext','C-grok':'Zig 0.15.2','D-gemini':'Kotlin/JVM 21','E-kimi':'C#/.NET 8','F-glm':'Swift 6.1.2 + C kernels'}
committed={}
for t in glob.glob(R+'/trees/*.json'):
    k=os.path.basename(t)[:-5]
    for e in json.load(open(t))['entries']:
        if e['type']=='blob':
            b=os.path.basename(e['path'])
            if '/candidates/' in '/'+e['path']: committed.setdefault(k,{})[b]=e['sha']
            if '/objects/' in '/'+e['path']:
                p=os.path.join(R,'src',k,e['path'])
                committed.setdefault(k+':obj',{})[b]=hashlib.sha256(open(p,'rb').read()).hexdigest() if os.path.exists(p) else None
def blob(p):
    b=open(p,'rb').read(); return hashlib.sha1(b'blob %d\0'%len(b)+b).hexdigest()
def rd(p):
    try: return open(p,errors='replace').read()
    except: return ''
def ms(v):
    v=[x for x in v if x is not None]
    return (st.median(v),min(v),len(v)) if v else None
def fmt(t): return '—' if not t else f'{t[0]:.2f} / {t[1]:.2f}'+('' if t[2]>=3 else f' (n={t[2]})')
def timings(M,i):
    O=f'{M}/out/{i}'; r={'fo':[],'fs':[],'lo':[],'ls':[],'note':''}
    if i=='C-grok':
        for f in sorted(glob.glob(O+'/bench*.log')):
            s=rd(f); g=lambda k: float(re.search(k+r'=([\d.]+)',s).group(1)) if re.search(k+r'=([\d.]+)',s) else None
            r['fo'].append(g('terrain_fast_s')); r['fs'].append(g('terrain_scalar_s')); r['lo'].append(g('leg_fast_s')); r['ls'].append(g('leg_scalar_s'))
        r['note']='1 thread; fast=@Vector path'
    elif i=='B-sol':
        for f in sorted(glob.glob(O+'/bench*.log')):
            for line in rd(f).splitlines():
                if line.startswith('{'):
                    d=json.loads(line); r['fo']+=d['terrain_optimized_samples']; r['fs']+=d['terrain_scalar_samples']; r['lo']+=d['leg_optimized_samples']; r['ls']+=d['leg_scalar_samples']; r['note']='backend='+d['backend']+'; 1 thread'
    elif i=='A-sonnet':
        lw=[]
        for f in sorted(glob.glob(O+'/bench*.log')):
            s=rd(f)
            def g(lbl):
                m=re.search(re.escape(lbl)+r'\s+([\d.]+) s',s); return float(m.group(1)) if m else None
            r['fs'].append(g('plain TypeScript, 1 thread, no WebAssembly')); r['fo'].append(g('WebAssembly kernel, 1 thread'))
            r['ls'].append(g('plain TypeScript (32-bit limbs) + node:crypto SHA-256')); r['lo'].append(g('WebAssembly fold + node:crypto SHA-256 (SHA-NI)')); lw.append(g('WebAssembly fold + WebAssembly software SHA-256'))
        t=ms(lw); r['note']='1 thread; opt leg = WASM fold + node:crypto SHA'+(f'; WASM fold + WASM SHA leg {fmt(t)}' if t else '')
    elif i=='D-gemini':
        for f in sorted(glob.glob(O+'/bench*.log')):
            s=rd(f)
            def g(lbl):
                m=re.search(re.escape(lbl)+r'\s+([\d.]+) ms',s); return float(m.group(1))/1000 if m else None
            r['fs'].append(g('Plain scalar terrain fill (1 thread):')); r['fo'].append(g('Optimised unrolled (4 threads) fill:'))
            r['ls'].append(g('Plain scalar production leg:')); r['lo'].append(g('Optimised C2 SuperWord AVX-512 leg:'))
        r['note']='each run = its own min of 2-3 reps; opt fill uses 4 threads; Vector-API-512 path emulated on AVX2 (not used)'
    elif i=='F-glm':
        for line in rd(O+'/bench-harness.log').splitlines():
            g=lambda k: float(re.search(k+r'=([\d.]+)',line).group(1)) if re.search(k+r'=([\d.]+)',line) else None
            if 'fill_' in line: r['fs'].append(g('fill_scalar_s')); r['fo'].append(g('fill_dispatch_s'))
            if 'leg_' in line: r['ls'].append(g('leg_scalar_s')); r['lo'].append(g('leg_avx2_s'))
        r['note']='external C harness on its compiled kernels (own bench SIGILLs w/o AVX-512); opt fill = dispatch (scalar w/o AVX-512); opt leg = AVX2 fold'
    elif i=='E-kimi':
        for f in sorted(glob.glob(O+'/bench*.log')):
            s=rd(f)
            def g(e,w):
                m=re.search(r'engine '+e+': '+w+r' ([\d.]+)s',s); return float(m.group(1)) if m else None
            r['fs'].append(g('Scalar','terrain fill')); r['fo'].append(g('Avx2','terrain fill'))
            r['ls'].append(g('Scalar','one production leg')); r['lo'].append(g('Avx2','one production leg'))
            m=re.search(r'seal backend: (\S+)',s)
            if m: r['note']='1 thread; opt = Avx2 engine; seal backend='+m.group(1)
    return {k:(ms(v) if k!='note' else v) for k,v in r.items()}
def tests(M,i):
    O=f'{M}/out/{i}'; s=rd(O+'/test.log')
    if i=='A-sonnet':
        m=re.findall(r'# (pass|fail) (\d+)',s); d=dict(m); return f"{d.get('pass','?')} pass / {d.get('fail','?')} fail" if d else '?'
    if i=='B-sol':
        m=re.search(r'Ran (\d+) tests',s); return (m.group(1)+' run, '+('OK' if '\nOK' in s else 'FAIL')+(' (skipped present)' if 'skipped' in s and 'OK (skipped' in s else '')) if m else '?'
    if i=='C-grok':
        m=re.search(r'(\d+)/(\d+) tests passed',s); return f'{m.group(1)}/{m.group(2)} pass' if m else ('FAIL' if 'failed' in s else '?')
    if i=='D-gemini':
        n=f=0
        for x in glob.glob(f'{M}/gemini-test-results/*.xml'):
            m=re.search(r'tests="(\d+)" skipped="(\d+)" failures="(\d+)" errors="(\d+)"',rd(x)); n+=int(m.group(1)); f+=int(m.group(3))+int(m.group(4))
        ok='BUILD SUCCESSFUL' in s
        return f'{n-f}/{n} pass' if n else ('PASS' if ok else 'FAIL')
    if i=='F-glm':
        a='SIGILL (prodTerrainScalarEqualsAvx512 calls AVX-512 kernel directly)' if 'Illegal instruction' in s else ('PASS' if 'passed' in s else '?')
        k=rd(O+'/test-skip.log'); m=re.search(r'Test run with (\d+) tests passed',k)
        return f'full run: {a}; with that test skipped: {m.group(1)}/{m.group(1)} pass' if m else a
    if i=='E-kimi':
        m=re.search(r'(Passed|Failed)!\s+-\s+Failed:\s+(\d+), Passed:\s+(\d+), Skipped:\s+(\d+), Total:\s+(\d+)',s)
        a=f'{m.group(3)}/{m.group(5)} pass' if m else '?'
        if m and m.group(2)!='0' and 'Avx512F' in s: a+=' (2 EngineEquivalence tests call the AVX-512 engine directly: PlatformNotSupportedException)'
        k=rd(O+'/test-skip.log'); n=re.search(r'Passed!\s+-\s+Failed:\s+0, Passed:\s+(\d+)',k)
        return f'full run: {a}; with those 2 excluded: {n.group(1)}/{n.group(1)} pass' if n else a
    return '—'
OUTK=('result','results','index_comparison','seed_value','shape')
def classify(p):
    s=rd(p).strip()
    if not s: return 'NO-RECORD',[],None
    try: d=json.loads(s)
    except Exception: return 'BAD-JSON',[],None
    bad=[];n=[0]
    def walk(x,path,key):
        if isinstance(x,dict):
            for k,v in x.items(): walk(v,path+'.'+k,k)
        elif isinstance(x,list):
            for j,v in enumerate(x): walk(v,f'{path}[{j}]',key)
        elif key in OUTK:
            n[0]+=1
            if x!='match': bad.append(f'{path}={x}')
    walk(d,'',None)
    canon=json.dumps(d,sort_keys=True,separators=(',',':'))
    return ('ALL-MATCH(%d)'%n[0] if not bad else 'NON-MATCH'),bad,hashlib.sha256(canon.encode()).hexdigest()[:12]
def machine(M):
    out={'ids':[i for i in IDS if os.path.isdir(f'{M}/out/{i}')]}
    out['time']={i:timings(M,i) for i in out['ids']}; out['tests']={i:tests(M,i) for i in out['ids']}
    out['cands']={i:{os.path.basename(f):blob(f) for f in glob.glob(f'{M}/out/{i}/cands/*.txt')} for i in out['ids']}
    out['objs']={i:{os.path.basename(f):hashlib.sha256(open(f,'rb').read()).hexdigest() for f in glob.glob(f'{M}/out/{i}/objects/*.cbor')} for i in out['ids']}
    g={}
    for f in glob.glob(f'{M}/xv/*.json'):
        v,_,p,k=os.path.basename(f)[:-5].split('.'); 
        c,b,h=classify(f); g[(v,p,k)]=(c,b,h)
    out['grid']=g; return out
if __name__=='__main__':
    for M in sys.argv[1:]:
        r=machine(M); print('##',M); 
        for i in r['ids']: print(i,r['tests'][i],{k:(fmt(v) if k!='note' else v) for k,v in r['time'][i].items()})
        print(collections.Counter(v[0] for v in r['grid'].values())); print(collections.Counter((k[2],v[2]) for k,v in r['grid'].items()))
        for k,v in sorted(r['grid'].items()):
            if v[0]!='ALL-MATCH': print(k,v[0],v[1][:5])
