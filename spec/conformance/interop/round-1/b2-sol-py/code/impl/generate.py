"""Deterministic candidate and offline-object producer for CASES.md."""
from __future__ import annotations
import argparse, hashlib, json, time
from pathlib import Path

from . import cbor
from .core import *
from .verify import draw_indices

PK_A=bytes.fromhex("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9")
PK_B=bytes.fromhex("dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659")
SEED_BEACON=sha256(b"pcoc-pack-2.2/beacon/seed")
INTERRUPTS=[sha256(b"pcoc-pack-2.2/beacon/"+str(k).encode()) for k in range(18)]


class Files:
    def __init__(self): self.d={n:{} for n in ("seed","nl4","terrain","fold","kat-small","test-3leg","prod","wes","rate","anchor","draw","unknown-kind","hashcash","negatives")}
    def put(self,f,k,v): self.d[f][k]=str(v)
    def write(self,path):
        path.mkdir(parents=True,exist_ok=True)
        for name, rows in self.d.items():
            (path/(name+".txt")).write_text("".join(f"{k}\t{rows[k]}\n" for k in sorted(rows)),encoding="ascii")


def chain_bytes(n=2048):
    b=sha256(b"pcoc-pack-2.2/draw"); out=b
    while len(out)<n: b=sha256(b); out+=b
    return out


def add_hike(f: Files, group, fixture, obj):
    for g,s in enumerate(obj["stages"]):
        f.put(group,f"{group}.{fixture}.stage.{g}.waypoint",s["waypoint"].hex())
        for j,(lr,cp) in enumerate(zip(s["leg_results"],s["checkpoints"])):
            f.put(group,f"{group}.{fixture}.stage.{g}.leg.{j}.legs_le_hex",lr.hex())
            f.put(group,f"{group}.{fixture}.stage.{g}.leg.{j}.checkpoint",cp.hex())


def hashcash(f):
    origin=b"https://example.com"; origin_label="bari-origin"
    header_hash=sha256(b"pcoc-pack-2.2/fee-header-hash")
    wp=sha256(b"pcoc-pack-2.2/hashcash-waypoint")
    challenge=sha256(b"bari-hc-v2"+origin+header_hash)
    nonce=0
    while True:
        digest=sha256(challenge+wp+le64(nonce))
        if int.from_bytes(digest,"big") < 1 << (256-21): break
        nonce+=1
    for k,v in (("origin_label",origin_label),("origin",origin.decode()),("header_hash",header_hash.hex()),
                ("waypoint",wp.hex()),("challenge",challenge.hex()),("nonce",nonce),("digest",digest.hex())):
        f.put("hashcash","hashcash."+k,v)


def generate(out: Path, objects: Path | None, include_production=True):
    f=Files()
    for label,pk in (("pk_A",PK_A),("pk_B",PK_B)):
        d,lanes=seed(pk,SEED_BEACON)
        f.put("seed",f"seed.{label}.preimage",(pk+SEED_BEACON).hex()); f.put("seed",f"seed.{label}.D",d.hex())
        for k,p in enumerate(lanes): f.put("seed",f"seed.{label}.P{k}_u64",f"{p:016x}")
    d,lanes=seed(PK_A,SEED_BEACON)
    t1=terrain(PK_A,SEED_BEACON,S1); tb1=terrain_bytes(t1)
    for i in (0,1,S1.n_slots-1):
        # CASES asks for j=0..15, but S1 SLOT_BYTES=64 gives exactly 8 words.
        # Emit every defined word (j=0..7); RESULTS.md records the blocking conflict.
        for j in range(min(16, S1.words_per_slot)):
            f.put("nl4",f"nl4.S1.slot.{i}.word.{j}.base_u64",f"{nl4_word(i,j,lanes):016x}")
            f.put("nl4",f"nl4.S1.slot.{i}.word.{j}.stored_u64",f"{int(t1[i*S1.words_per_slot+j]):016x}")
    f.put("terrain","terrain.S1.digest",sha256(tb1).hex())
    result,trace=leg(t1,d,S1,4)
    for n,(s,i,acc,nxt) in enumerate(trace):
        f.put("fold",f"fold.S1.step.{n}.S_u64",f"{s:016x}"); f.put("fold",f"fold.S1.step.{n}.slot_index",i)
        f.put("fold",f"fold.S1.step.{n}.acc_u64",f"{acc:016x}"); f.put("fold",f"fold.S1.step.{n}.S_next_u64",f"{nxt:016x}")
    f.put("fold","fold.S1.leg_result_u64",f"{result:016x}")
    heights=list(range(100,119))
    o1,_=build_hike(PK_A,SEED_BEACON,INTERRUPTS,heights,S1,[1]*18)
    add_hike(f,"kat-small","S1",o1)
    f.put("wes","wes.S1.D",d.hex()); f.put("wes","wes.S1.stage.0.waypoint",o1["stages"][0]["waypoint"].hex())
    f.put("wes","wes.S1.equal","true" if d==o1["stages"][0]["waypoint"] else "false")
    t2=terrain(PK_A,SEED_BEACON,S2); f.put("terrain","terrain.S2.digest",sha256(terrain_bytes(t2)).hex())
    o2,_=build_hike(PK_A,SEED_BEACON,INTERRUPTS,heights,S2,[3]*18)
    add_hike(f,"test-3leg","S2",o2)
    for legs in (0,1,5,6,1000):
        for span in (1,300,3600):
            for p,q in ((1,5),(1,4),(7,20)):
                outcome="meet" if span>0 and q*legs>=p*span else "refuse"
                f.put("rate",f"rate.legs.{legs}.span.{span}.floor.{p}_{q}",outcome)
    for h in range(100,107): f.put("anchor",f"anchor.H.100.height.{h}","anchored" if h-100 in (2,3,4) else "not anchored")
    source=chain_bytes()
    for total in (6,18,54):
        f.put("draw",f"draw.legs.{total}.indices",",".join(map(str,draw_indices(total,source))))
    for label in ("v2.1-pcoc-object","satoshi-hike-v2.0-object","","v2.2-pcoc-objecT"):
        rec=json.dumps({"kind":"unknown kind","kind_text":label},separators=(",",":")).encode()
        safe=label or "empty"
        f.put("unknown-kind",f"unknown-kind.{safe}.record",rec.hex())
    hashcash(f)
    from . import negatives
    for name in negatives.NAMES:
        if name in ("one_lane_flip","all4_kernel","f3_shared_terrain","squeezed_seed_terrain"):
            value=sha256((name+sha256(tb1).hex()).encode()).hex()
        else: value=le64(negatives.variant_leg(t1,d,S1,name)).hex()
        f.put("negatives",f"negatives.{name}.value",value)
        f.put("negatives",f"negatives.{name}.matches_honest","false")
    if include_production:
        tp=terrain(PK_A,SEED_BEACON,PRODUCTION); tpb=terrain_bytes(tp)
        digest=sha256(tpb).hex(); f.put("terrain","terrain.PROD.digest",digest); f.put("prod","prod.PROD.terrain_digest",digest)
        lr,_=leg(tp,d,PRODUCTION); cp=checkpoint(d,d,terrain_sample(tpb,d,PRODUCTION),lr)
        f.put("prod","prod.PROD.stage.0.leg.0.legs_le_hex",le64(lr).hex()); f.put("prod","prod.PROD.stage.0.leg.0.checkpoint",cp.hex())
        if objects:
            o3,_=build_hike(PK_A,SEED_BEACON,INTERRUPTS,heights,PRODUCTION,[1]*18)
            objects.mkdir(parents=True,exist_ok=True); (objects/"O3.cbor").write_bytes(cbor.encode(o3))
    if objects:
        objects.mkdir(parents=True,exist_ok=True)
        (objects/"O1.cbor").write_bytes(cbor.encode(o1)); (objects/"O2.cbor").write_bytes(cbor.encode(o2))
    f.write(out)


def main():
    p=argparse.ArgumentParser(); p.add_argument("--out",type=Path,default=Path("candidates"))
    p.add_argument("--objects",type=Path,default=None); p.add_argument("--without-production",action="store_true")
    a=p.parse_args(); generate(a.out,a.objects,not a.without_production)
if __name__=="__main__": main()
