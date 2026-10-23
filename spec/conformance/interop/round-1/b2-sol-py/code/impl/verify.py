"""Section 14 offline verifier (portable-index documents are injectable)."""
from __future__ import annotations

import json
import secrets
from typing import Callable

from . import cbor
from .core import (KIND, STAGES, Profile, PRODUCTION, checkpoint, leg, seed,
                   terrain, terrain_bytes, terrain_sample, u64_le, waypoint)

ND, NR = "not defined", "not read"


def draw_indices(total: int, source: bytes | None = None) -> list[int] | str:
    if total < 6: return ND
    chosen, pos = [], 0
    limit = 2**64 - (2**64 % total)
    while len(chosen) < 6:
        if source is None: b = secrets.token_bytes(8)
        else:
            if pos + 8 > len(source): return NR
            b, pos = source[pos:pos+8], pos + 8
        u = int.from_bytes(b, "little")
        if u < limit and u % total not in chosen: chosen.append(u % total)
    return chosen


def _shape(o, p: Profile) -> list[str]:
    issues = []
    top = {"kind","fill","slot_bytes","n_slots","sample_bytes","steps_per_leg",
           "public_key","seed_beacon","stages","last_interrupt","last_interrupt_height"}
    if type(o) is not dict or set(o) != top: issues.append("field set")
    stages = o.get("stages", []) if type(o) is dict else []
    if len(stages) != 18: issues.append("stages length")
    if stages and "path" in stages[0]: issues.append("stage-0 path key")
    if len(stages) < 18 or stages[17].get("path") != b"": issues.append("W_17 path")
    if any(o.get(k) != v for k,v in (("kind",KIND),("fill","dag2"),
       ("slot_bytes",p.slot_bytes),("n_slots",p.n_slots),
       ("sample_bytes",p.sample_bytes),("steps_per_leg",p.steps_per_leg))):
        issues.append("constant")
    leg_bad = cp_bad = lr_bad = False
    for g,s in enumerate(stages[:18]):
        expected = {"beacon","beacon_height","leg_count","waypoint","checkpoints","leg_results"}
        if g: expected.add("path")
        if type(s) is not dict or set(s) != expected: 
            if "field set" not in issues: issues.append("field set")
            continue
        cps, lrs = s["checkpoints"], s["leg_results"]
        leg_bad |= type(cps) is not list or type(lrs) is not list or s["leg_count"] < 1 or len(cps) != s["leg_count"] or len(lrs) != len(cps)
        cp_bad |= any(type(x) is not bytes or len(x) != 32 for x in cps)
        lr_bad |= any(type(x) is not bytes or len(x) != 8 for x in lrs)
    if leg_bad: issues.append("leg_count")
    if cp_bad: issues.append("checkpoint width")
    if lr_bad: issues.append("leg result width")
    if not stages or stages[0].get("beacon") != o.get("seed_beacon"): issues.append("seed beacon")
    headers = [s.get("beacon") for s in stages[:18]] + [o.get("last_interrupt")]
    heights = [s.get("beacon_height") for s in stages[:18]] + [o.get("last_interrupt_height")]
    if len(stages)<18 or type(o.get("last_interrupt")) is not bytes or len(o.get("last_interrupt",b"")) != 32 or not all(type(x) is int for x in heights):
        issues.append("last_interrupt")
    else:
        if heights[-1] <= heights[-2] or headers[-1] == headers[-2]: issues.append("last_interrupt")
        if any(b <= a for a,b in zip(heights, heights[1:])): issues.append("header height order")
        if any(a == b for a,b in zip(headers, headers[1:])): issues.append("header hash differ")
        if any(x > 2**53-1 for x in heights): issues.append("height_range")
    order = ["malformed cbor","field set","stage-0 path key","W_17 path","constant",
             "stages length","leg_count","checkpoint width","leg result width",
             "seed beacon","last_interrupt","header height order","header hash differ","height_range"]
    return [x for x in order if x in issues]


def verify_bytes(data: bytes, profile: Profile = PRODUCTION, profile_name="production",
                 index: Callable[[int], dict | None] | None = None,
                 draw_bytes: bytes | None = None, recompute_six=True):
    try: o = cbor.decode(data)
    except cbor.DecodeError:
        return _empty(profile, profile_name, ["malformed cbor"])
    if type(o) is not dict or type(o.get("kind")) is not str: raise cbor.DecodeError("no kind")
    if o["kind"] != KIND: return {"kind":"unknown kind","kind_text":o["kind"]}
    issues = _shape(o, profile)
    terr = terrain(o["public_key"], o["seed_beacon"], profile)
    tb = terrain_bytes(terr)
    d,_ = seed(o["public_key"],o["seed_beacon"])
    headers = []
    for s in o["stages"] + [{"beacon":o["last_interrupt"],"beacon_height":o["last_interrupt_height"]}]:
        doc = index(s["beacon_height"]) if index else None
        comparison, tm = NR, NR
        if doc and type(doc.get("header")) is dict:
            h=doc["header"]; expected=s["beacon"][::-1].hex()
            comparison="match" if h.get("hash","").lower()==expected and h.get("height")==s["beacon_height"] else "mismatch"
            if comparison=="match" and type(h.get("time")) is int and 0<=h["time"]<2**32: tm=h["time"]
        headers.append({"stored_hash":s["beacon"].hex(),"stored_height":s["beacon_height"],"index_comparison":comparison,"T":tm})
    cps_record=[]; waypoints=[]
    for g,s in enumerate(o["stages"]):
        legs=[]; previous=d if g==0 else s["waypoint"]; limb=previous
        for j,cp in enumerate(s["checkpoints"]):
            ok=j<len(s["leg_results"]) and cp==checkpoint(previous,limb,terrain_sample(tb,previous,profile),u64_le(s["leg_results"][j]))
            legs.append({"j":j,"result":"match" if ok else "mismatch"}); previous=cp
        cps_record.append({"g":g,"legs":legs})
        if g<17:
            got=waypoint(o["stages"][g+1]["beacon"],tb,s["waypoint"],s["checkpoints"],profile)
            waypoints.append({"g":g+1,"result":"match" if got==o["stages"][g+1]["waypoint"] else "mismatch"})
    paths=[]; anchors=[]; skips=[]
    for g in range(1,17):
        state="skip" if o["stages"][g]["path"]==b"" else "present"
        if state=="skip": skips.append(g)
        paths.append({"g":g,"path_state":state,"plant":"n/a" if state=="skip" else NR,
                      "P_g":ND,"height_P_g":ND,"d_g":ND,"claimed_block":ND})
        anchors.append("not anchored")
    paths.append({"g":17,"w17_path":"not fetched by design","claimed_block":ND})
    total=sum(len(s["checkpoints"]) for s in o["stages"])
    indices=draw_indices(total,draw_bytes)
    six, runs = indices, []
    if isinstance(indices,list):
        results=[]
        starts=[]; n=0
        for g,s in enumerate(o["stages"]):
            for j in range(len(s["checkpoints"])): starts.append((g,j))
        for q in indices:
            g,j=starts[q]; s=o["stages"][g]
            previous=(d if g==0 else s["waypoint"]) if j==0 else s["checkpoints"][j-1]
            result,_=leg(terr,previous,profile) if recompute_six else (u64_le(s["leg_results"][j]),[])
            results.append("match" if result==u64_le(s["leg_results"][j]) else "mismatch")
        six={"indices":indices,"results":results}; runs=[six]
    elif indices==NR: runs=NR
    per=[{"g":g,"checkpoint_count":len(s["checkpoints"]),"span":NR} for g,s in enumerate(o["stages"])]
    record={"profile":profile_name}
    if profile_name=="conformance-small":
        record.update(sample_bytes=profile.sample_bytes,slot_bytes=profile.slot_bytes,
                      terrain_bytes=profile.terrain_bytes,steps_per_leg=profile.steps_per_leg)
    record.update(shape="match" if not issues else "mismatch",shape_item=issues,headers=headers,
      seed_value="match" if o["stages"][0]["waypoint"]==d else "mismatch",
      checkpoints=cps_record,waypoints=waypoints,paths=paths,anchors=anchors,
      anchored_total=0,skip_list=skips,a=ND,b=ND,N=ND,S=ND,delta_h=ND,
      fixed_start={"N":ND,"S":ND,"delta_h":ND},P_b=ND,height_P_b=ND,
      index_comparison_P_b=ND,T_P_b=ND,height_I_a_minus_1=ND,T_I_a_minus_1=ND,
      six_leg=six,six_leg_runs=runs,
      analysis={"whole_hike_legs":sum(len(s["checkpoints"]) for s in o["stages"][1:]),
                "whole_hike_span":NR,"per_stage":per,"legs_outside":ND,
                "hike_legs":total,"hike_span":NR})
    return record


def _empty(p, name, issues):
    # Malformed input still has the fixed record shape.
    fake={"kind":KIND,"fill":"", "slot_bytes":0,"n_slots":0,"sample_bytes":0,
          "steps_per_leg":0,"public_key":b"","seed_beacon":b"","stages":[],
          "last_interrupt":b"","last_interrupt_height":0}
    r={"profile":name}
    if name=="conformance-small": r.update(sample_bytes=p.sample_bytes,slot_bytes=p.slot_bytes,terrain_bytes=p.terrain_bytes,steps_per_leg=p.steps_per_leg)
    r.update(shape="mismatch",shape_item=issues,headers=[{"stored_hash":ND,"stored_height":ND,"index_comparison":NR,"T":NR} for _ in range(19)],
             seed_value="mismatch",checkpoints=[{"g":g,"legs":ND} for g in range(18)],
             waypoints=[{"g":g,"result":ND} for g in range(1,18)],
             paths=[({"g":g,"path_state":"skip","plant":"n/a","P_g":ND,"height_P_g":ND,"d_g":ND,"claimed_block":ND} if g<17 else {"g":17,"w17_path":ND,"claimed_block":ND}) for g in range(1,18)],
             anchors=["not anchored"]*16,anchored_total=0,skip_list=[],a=ND,b=ND,N=ND,S=ND,delta_h=ND,
             fixed_start={"N":ND,"S":ND,"delta_h":ND},P_b=ND,height_P_b=ND,index_comparison_P_b=NR,T_P_b=NR,
             height_I_a_minus_1=ND,T_I_a_minus_1=NR,six_leg=ND,six_leg_runs=[],
             analysis={"whole_hike_legs":ND,"whole_hike_span":NR,"per_stage":[{"g":g,"checkpoint_count":ND,"span":NR} for g in range(18)],"legs_outside":ND,"hike_legs":ND,"hike_span":NR})
    return r


def json_line(record) -> str:
    return json.dumps(record,separators=(",",":"),ensure_ascii=False)+"\n"
