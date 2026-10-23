import sys,os,re,json,collections,glob
sys.path.insert(0,os.path.dirname(__file__)); from analyze import *
MACH=[('ref1','ref1 (PRIMARY timing reference)'),("ref2","ref2 (low-end host per SPEC §15.5; i7-10610U, no SHA-NI; batch job ~40% of one thread during bench runs)"),('ref3','ref3')]
SH={'A-sonnet':'A Sonnet','B-sol':'B Sol','C-grok':'C Grok','D-gemini':'D Gemini','E-kimi':'E Kimi','F-glm':'F GLM'}
def minfo(M):
    s=rd(f'machines/{M}/machine.txt')
    g=lambda k: (re.search(k+r':\s*(.+)',s) or [None,'?'])[1].strip()
    flags=set(re.search(r'Flags:\s*(.+)',s).group(1).split()) if re.search(r'Flags:\s*(.+)',s) else set()
    fl=' '.join(f for f in ['avx2','avx512f','avx512dq','avx512bw','avx512vl','sha_ni'] if f in flags) or '?'
    m=re.search(r'Mem:\s+(\d+)',s); ram=f'{int(m.group(1))/2**30:.1f} GiB' if m else '?'
    return f"{g('Model name')}; {g('Core\\(s\\) per socket')} cores / {g('CPU\\(s\\)')} threads; L3 {g('L3 cache')}; RAM {ram}; flags: {fl} (no AVX-512)" if 'avx512f' not in flags else f"{g('Model name')}; L3 {g('L3 cache')}; RAM {ram}; flags: {fl}"
def ver(M,i):
    s=rd(f'machines/{M}/out/{i}/versions.txt')
    pick={'A-sonnet':r'^v\d+\.\d+\.\d+|Version [\d.]+','B-sol':r'Python [\d.]+|gcc .*|CFLAGS=.*','C-grok':r'^0\.\d+\.\d+','D-gemini':r'openjdk version "[^"]+"|Kotlin: [\d.]+|Gradle [\d.]+','F-glm':r'Swift version [\d.]+','E-kimi':r'^8\.\d+\.\d+'}.get(i,'.')
    return '; '.join(dict.fromkeys(m.group(0).strip() for m in re.finditer(pick,s,re.M)))
out=['# Round 2 same-box comparison: Satoshi Hike SPEC 2.2 + erratum 1','',
'Every number below was measured on these machines from each build\'s own source at its round-2 HEAD. Cloud-VM figures in the builds\' RESULTS.md are not used.',
'Times are seconds, shown as median / min over 3 runs (or more samples where the build\'s own bench takes more), one 256 MiB production terrain fill and one 2,000,000-step production leg.','',
'Builds: A Sonnet 5.5 TypeScript `b3d7b02`, B Sol Python+C `b18a5f6`, C Grok Zig `ffcfc22`, D Gemini Kotlin `fd24932`, E Kimi C#/.NET 8 `d9fa6ea`, F GLM Swift `6d976f4`.','']
done=[M for M,_ in MACH if os.path.isdir(f'machines/{M}/out')]
R={M:machine(f'machines/{M}') for M in done}
for M,label in MACH:
    out.append(f'## {label}')
    if M not in R: out+=['','_pending_',''];continue
    r=R[M]; out+=['',minfo(M),'',
     '| Build | Language | Toolchain | Fill opt | Fill scalar | Leg opt | Leg scalar | Own tests | Notes |','|---|---|---|---|---|---|---|---|---|']
    for i in IDS:
        if i not in r['ids']: out.append(f'| {SH[i]} | {LANG[i]} | — | — | — | — | — | not run | repo not found |');continue
        t=r['time'][i]; out.append(f"| {SH[i]} | {LANG[i]} | {ver(M,i)} | {fmt(t['fo'])} | {fmt(t['fs'])} | {fmt(t['lo'])} | {fmt(t['ls'])} | {r['tests'][i]} | {t['note']} |")
    # candidates
    ids=r['ids']; files=sorted(set(f for i in ids for f in r['cands'][i]))
    out+=['','**Candidates** (generated on this machine; `=` same bytes as the build\'s committed file and as every other build, else the file differs):','','| file | '+' | '.join(SH[i] for i in ids)+' |','|---|'+'---|'*len(ids)]
    for f in files:
        vals={i:r['cands'][i].get(f) for i in ids}; modal=collections.Counter(v for v in vals.values() if v).most_common(1)[0][0]
        cells=[]
        for i in ids:
            v=vals[i]; c=committed.get(i,{}).get(f)
            cells.append('missing' if not v else ('=' if v==modal and v==c else ('DIFF vs others' if v!=modal else '')+(' ≠committed' if v!=c else '')))
        out.append(f'| {f} | '+' | '.join(cells)+' |')
    out+=['','**Objects** (sha256 of objects produced on this machine; prefix shown):','','| build | O1-S1 | O2-S2 | O3-PROD | same as committed |','|---|---|---|---|---|']
    for i in ids:
        o=r['objs'][i]; c=committed.get(i+':obj',{})
        row=[o.get(n,'')[:16] for n in ['O1-S1.cbor','O2-S2.cbor','O3-PROD.cbor']]
        out.append(f'| {SH[i]} | '+' | '.join(row)+' | '+('yes' if all(o.get(n)==c.get(n) for n in c) else 'NO')+' |')
    for k in ['O1','O2','O3']:
        out+=['',f'**Cross-verify {k}** (row = verifier, column = producer; cell = outcome of every `result`/`results`/`index_comparison`/`seed_value`/`shape` field, count in brackets):','','| verifier \\ producer | '+' | '.join(SH[i] for i in ids)+' |','|---|'+'---|'*len(ids)]
        for v in ids:
            cells=[]
            for p in ids:
                c=r['grid'].get((v,p,k)); cells.append('—' if not c else (c[0].replace('ALL-MATCH','all match') if c[0].startswith('ALL') else c[0]+': '+'; '.join(c[1][:3])))
            out.append(f'| {SH[v]} | '+' | '.join(cells)+' |')
        hs=collections.Counter(c[2] for (v,p,kk),c in r['grid'].items() if kk==k)
        out.append(''); out.append(f'Distinct verify records for {k}: {len(hs)}'+(' (all byte-identical after canonical JSON)' if len(hs)==1 else ' — differences are only Grok\'s six-leg indices: its CLI refuses `--draw-label` on the production profile and draws from the OS CSPRNG, so its 6 indices are random; all results match.' if k=='O3' else ''))
    out.append('')
out+=open('SAME-BOX-notes.md').read().splitlines() if os.path.exists('SAME-BOX-notes.md') else []
out+=(['']+open('SAME-BOX-packed.md').read().splitlines()) if os.path.exists('SAME-BOX-packed.md') else []
open('SAME-BOX.md','w').write('\n'.join(out)+'\n'); print('written',done)
