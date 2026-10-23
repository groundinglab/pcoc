import glob,os,re,statistics as st,sys,csv
def runs(root):
    out=[]
    for d in sorted(glob.glob(root+'/packed/*-grok-*-N*')):
        if not os.path.isdir(d): continue
        m,b,e,n=re.match(r'(\w+)-(\w+)-(\w+)-N(\d+)$',os.path.basename(d)).groups()
        mem=open(d+'/mem.txt').read(); g=lambda k:int(re.search(k+r'=(\d+)',mem).group(1))
        t0,t1=g('T0'),g('T1'); rss=[int(x) for x in re.findall(r'rss_kb_\d+=(\d+)',mem)]
        ver={}
        if os.path.exists(d+'/verify.csv'):
            for l in open(d+'/verify.csv'):
                p=l.strip().split(','); ver[p[0]]=(p[1],p[4])
        rows=[]
        for f in sorted(glob.glob(d+'/i*.log')):
            ends=[int(l.split(',')[3]) for l in open(f) if l.startswith('leg,')]
            w=[x for x in ends if t0<=x<=t1]
            r=(len(w)-1)/((w[-1]-w[0])/1e9) if len(w)>1 else 0
            i=os.path.basename(f)[:-4]
            rows.append(dict(machine=m,build=b,engine=e,N=int(n),instance=i,legs_total=len(ends),legs_in_window=len(w)-1,window_s=round((w[-1]-w[0])/1e9,2) if len(w)>1 else 0,legs_per_s=round(r,4),rss_kb=rss[int(i[1:])-1] if len(rss)>=int(i[1:]) else '',verify_leg=ver.get(i,('',''))[0],verify=ver.get(i,('','not run'))[1]))
        load=re.search(r'load average: ([\d.]+, [\d.]+, [\d.]+)',mem)
        out.append(dict(m=m,e=e,N=int(n),rows=rows,used=g('mem_mid_used'),avail=int(re.search(r'mem_mid_used=\d+ avail=(\d+)',mem).group(1)),swap=g('swap_used'),load=load.group(1) if load else ''))
    return out
if __name__=='__main__':
    R=runs(sys.argv[1]); M=sys.argv[2]
    rows=[r for x in R for r in x['rows']]
    with open(f'packed-{M}.csv','w',newline='') as f:
        w=csv.DictWriter(f,fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)
    for x in R:
        v=[r['legs_per_s'] for r in x['rows']]
        print(f"{x['m']} {x['e']} N={x['N']} med={st.median(v):.3f} min={min(v):.3f} agg={sum(v):.3f} rss_MiB={[round(r['rss_kb']/1024) for r in x['rows'] if r['rss_kb']!='']} used_GiB={x['used']/2**30:.2f} avail_GiB={x['avail']/2**30:.2f} swap={x['swap']} load={x['load']} verify={[r['verify'] for r in x['rows']]}")
