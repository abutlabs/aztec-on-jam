#!/usr/bin/env python3
"""The queue soak: how many packages one core takes a slot when packages are waiting for it.

soak.py chains proofs, so it sends a package only once the one before is reported, and
measures one package at a time. Here the packages are independent public-balance mints, so
several can wait for the core at once, as when the demo seeds its accounts (lasair#94).

  client/queue_soak.py [--seconds 300] [--queue 3] [--core 0] [--out out/queue-soak]

Package k mints k+1 to the soak's own account, so its event names it. --queue packages are
kept outstanding on --core. One not landed within --expire-after seconds is sent again with
the same payload: the same nonce, so the service applies it once however many copies land.
The report gives the pace (landings per slot against one a slot), the latency in slots and
seconds, the resends, and the gaps between landings. The verdict (exit 0) needs every package
landed, none twice, and a pace of at least --min-pace.

Writes <out>/events.jsonl, <out>/report.json and <out>/report.md.
"""
import argparse, hashlib, json, os, struct, time, urllib.request
from pathlib import Path

INDEXER = os.environ.get("INDEXER", "http://localhost:19700")
ACCOUNT = bytes([0]) + hashlib.sha256(b"jam-privacy-queue-soak").digest()[1:]   # a field element


def http(method, path, body=None, timeout=120):
    data = None if body is None else body.encode()
    with urllib.request.urlopen(urllib.request.Request(INDEXER + path, data=data, method=method), timeout=timeout) as r:
        return json.loads(r.read())


def mint(amount, nonce):
    """public-balance op 0: [0][account: 32][amount: u64 LE][nonce: 32]"""
    return bytes([0]) + ACCOUNT + struct.pack("<Q", amount) + nonce


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--seconds", type=int, default=300)
    ap.add_argument("--queue", type=int, default=3)
    ap.add_argument("--core", type=int, default=0)
    ap.add_argument("--slot", type=float, default=6.0)
    ap.add_argument("--expire-after", type=float, default=60.0)
    ap.add_argument("--min-pace", type=float, default=0.9)
    ap.add_argument("--out", default="out/queue-soak")
    a = ap.parse_args()
    out = Path(a.out); out.mkdir(parents=True, exist_ok=True)
    evf = (out / "events.jsonl").open("w")

    def log(kind, **kw):
        kw.update(t=round(time.time(), 3), kind=kind)
        evf.write(json.dumps(kw) + "\n"); evf.flush()

    st = http("GET", "/state")
    pub, net = st["public_id"], st.get("net", "")
    acct = ACCOUNT.hex()
    seen = {e["n"] for e in http("GET", "/history")["events"]}      # what was there before
    pk = {}            # k -> {"payload", "sends": [(t, slot)], "landed": (t, slot) or None}
    landed_twice = 0
    k_next = 0
    t_start = time.time(); slot_start = http("GET", "/head")["best"]
    while time.time() - t_start < a.seconds:
        now = time.time(); head = http("GET", "/head")["best"]
        # landings: mint events to our account, amount k+1
        for e in http("GET", "/history")["events"]:
            if e["n"] in seen:
                continue
            seen.add(e["n"])
            if e["kind"] == "mint" and e["account"] == acct:
                k = e["amount"] - 1
                if k in pk and pk[k]["landed"] is None:
                    pk[k]["landed"] = (now, e["slot"]); log("landed", k=k, slot=e["slot"])
                else:
                    landed_twice += 1; log("landed_again", k=k, slot=e["slot"])
        outstanding = [k for k, p in pk.items() if p["landed"] is None]
        # a package not landed in time: the same payload again, in a freshly anchored package
        for k in outstanding:
            if now - pk[k]["sends"][-1][0] >= a.expire_after:
                r = http("POST", f"/submit/{pub}?core={a.core}", pk[k]["payload"].hex())
                pk[k]["sends"].append((time.time(), head)); log("resend", k=k, package=r.get("package_hash", "")[:16], slot=head)
        while len([k for k, p in pk.items() if p["landed"] is None]) < a.queue and time.time() - t_start < a.seconds:
            k = k_next; k_next += 1
            payload = mint(k + 1, os.urandom(32))
            r = http("POST", f"/submit/{pub}?core={a.core}", payload.hex())
            pk[k] = {"payload": payload, "sends": [(time.time(), head)], "landed": None}
            log("submit", k=k, package=r.get("package_hash", "")[:16], slot=head)
        time.sleep(1.0)
    # a last look for packages already sent
    deadline = time.time() + a.expire_after
    while time.time() < deadline and any(p["landed"] is None for p in pk.values()):
        now = time.time()
        for e in http("GET", "/history")["events"]:
            if e["n"] in seen:
                continue
            seen.add(e["n"])
            if e["kind"] == "mint" and e["account"] == acct:
                k = e["amount"] - 1
                if k in pk and pk[k]["landed"] is None:
                    pk[k]["landed"] = (now, e["slot"]); log("landed", k=k, slot=e["slot"])
                else:
                    landed_twice += 1
        time.sleep(1.0)
    slot_end = http("GET", "/head")["best"]

    landed = {k: p for k, p in pk.items() if p["landed"]}
    in_window = [p for p in landed.values() if p["landed"][1] <= slot_start + int(a.seconds / a.slot) + 1]
    slots = max(1, int(a.seconds / a.slot))
    lat_s = sorted(p["landed"][0] - p["sends"][0][0] for p in landed.values())
    lat_slots = sorted(p["landed"][1] - p["sends"][0][1] for p in landed.values())
    land_slots = sorted(p["landed"][1] for p in landed.values())
    gaps = [b - a_ for a_, b in zip(land_slots, land_slots[1:])]
    pct = lambda xs, q: xs[min(len(xs) - 1, int(q * len(xs)))] if xs else None
    rep = {
        "net": net, "core": a.core, "queue": a.queue, "seconds": a.seconds,
        "slots": [slot_start, slot_end], "submitted": len(pk), "landed": len(landed),
        "not_landed": len(pk) - len(landed), "landed_twice": landed_twice,
        "resends": sum(len(p["sends"]) - 1 for p in pk.values()),
        "packages_resent": sum(1 for p in pk.values() if len(p["sends"]) > 1),
        "pace_vs_one_a_slot": round(len(in_window) / slots, 2),
        "latency_s": {"p50": round(pct(lat_s, .5), 1), "p90": round(pct(lat_s, .9), 1), "max": round(lat_s[-1], 1)} if lat_s else None,
        "latency_slots": {"p50": pct(lat_slots, .5), "p90": pct(lat_slots, .9), "max": lat_slots[-1]} if lat_slots else None,
        "gap_slots_between_landings": dict(sorted({g: gaps.count(g) for g in set(gaps)}.items())),
    }
    rep["verdict"] = "PASS" if (rep["not_landed"] == 0 and landed_twice == 0 and rep["pace_vs_one_a_slot"] >= a.min_pace) else "FAIL"
    (out / "report.json").write_text(json.dumps(rep, indent=1))
    (out / "report.md").write_text("| measure | value |\n|---|---|\n" + "".join(f"| {k} | {v} |\n" for k, v in rep.items()))
    print((out / "report.md").read_text())
    return 0 if rep["verdict"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
