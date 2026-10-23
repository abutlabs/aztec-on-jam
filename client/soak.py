#!/usr/bin/env python3
"""The soak: one privacy package a slot, for an hour by default, against the running net and
its indexer. Each package is a real proof of a zero-value transfer by a soak user; they chain
(each starts from the root the one before left), so they go to one core, in order.

  client/soak.py [--seconds 3600] [--slot 6] [--core 0] [--ahead 4] [--out out/soak]

A prover thread keeps `ahead` proofs ready. The submitter is the builder: it sends package k+1
only once package k is Reported (JIP-2 workPackageStatus), and at most one a slot, because a
core holds one pending report at a time, so reported order is accumulation order. Sent
blind, one a slot, a late package lets the next be reported first; accumulate then refuses
its stale root and the chain of proofs breaks (the 2026-10-23 trial). A package not reported
within --report-after seconds is sent again (safe: the nullifiers make it apply once).
An observer records every root change and the best and finalized slots. A package whose root
never appears within --drop-after seconds is counted as dropped, the client resyncs from the
chain, and the soak goes on. The verdict (exit 0) needs: zero dropped packages, every package
landed, no proof that failed to build, no slot left without a package to send, at least
90% of one package a slot, and finality advancing the whole time (no stall longer than
--max-stall seconds).
Writes <out>/events.jsonl, <out>/report.json and <out>/report.md.
"""
import argparse, json, os, queue, statistics, sys, threading, time, urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import privacy  # noqa: E402

INDEXER = os.environ.get("INDEXER", "http://localhost:19700")
SOAK_USER = "soak"


def http(method, path, body=None, timeout=60):
    data = None if body is None else body.encode()
    with urllib.request.urlopen(urllib.request.Request(INDEXER + path, data=data, method=method), timeout=timeout) as r:
        return json.loads(r.read())


class Soak:
    def __init__(self, a):
        self.a = a
        self.out = Path(a.out); self.out.mkdir(parents=True, exist_ok=True)
        privacy.STATE = self.out / "state.json"
        privacy.STATE.unlink(missing_ok=True)
        self.lock = threading.Lock()            # the client's note state has one writer at a time
        self.ready = queue.Queue(maxsize=a.ahead)
        self.epoch = 0                          # bumped on a resync: older proofs are discarded
        self.txs = []                           # one dict per submitted package
        self.by_root = {}                       # new root hex -> tx
        self.heads = []                         # (t, best, finalized)
        self.stop = threading.Event()
        self.ev = (self.out / "events.jsonl").open("w")
        st = http("GET", "/state")
        self.pv, self.net = st["privacy_id"], st.get("net", "")

    def log(self, kind, **kw):
        kw.update(t=round(time.time(), 3), kind=kind)
        self.ev.write(json.dumps(kw) + "\n"); self.ev.flush()

    def sync(self):
        with self.lock:
            privacy.sync(http("GET", "/state"))

    def prover(self):
        while not self.stop.is_set():
            epoch = self.epoch
            try:
                with self.lock:
                    t0 = time.time()
                    r = privacy.transfer(SOAK_USER, "-", "-", SOAK_USER, 0, 0, 0, 0)
                    dt = time.time() - t0
            except privacy.ClientError as e:
                self.log("prove_error", error=str(e)[:300]); time.sleep(2); continue
            item = {"payload": r["payload"].hex(), "old": f"{r['fields'][0]:064x}",
                    "new": f"{r['fields'][2]:064x}", "prove_s": round(dt, 3), "epoch": epoch}
            while not self.stop.is_set():
                try:
                    self.ready.put(item, timeout=1); break
                except queue.Full:
                    continue

    def observer(self):
        last_root = None
        while not self.stop.is_set():
            try:
                st = http("GET", "/state"); hd = http("GET", "/head")
                now = time.time()
                self.heads.append((now, hd["best"], hd["finalized"]))
                root = st.get("note_root")
                if root != last_root:
                    tx = self.by_root.get(root)
                    if tx:
                        # the packages chain, so this root means every earlier one landed too,
                        # even if two landed between polls
                        for t in self.txs[: tx["k"] + 1]:
                            if t.get("landed") is None and not t.get("dropped"):
                                t["landed"] = now; t["latency_s"] = round(now - t["submitted"], 2)
                                t["slot_landed"] = st.get("slot")
                    self.log("root", root=root[:16] if root else None, leaves=st.get("leaf_count"),
                             transfers=st.get("transfers"), slot=st.get("slot"),
                             ours=bool(tx), finalized=hd["finalized"])
                    last_root = root
            except Exception as e:  # noqa: BLE001 — record and keep watching
                self.log("observe_error", error=str(e)[:200])
            time.sleep(self.a.poll)

    def resync(self, why):
        self.log("resync", why=why)
        time.sleep(self.a.slot * 3)              # let the in-flight packages settle either way
        self.epoch += 1
        while not self.ready.empty():
            try: self.ready.get_nowait()
            except queue.Empty: break
        self.sync()

    def run(self):
        a = self.a
        self.sync()
        start_head = http("GET", "/head")
        threading.Thread(target=self.prover, daemon=True).start()
        threading.Thread(target=self.observer, daemon=True).start()
        t_start = time.time(); next_at = t_start; last_report = t_start
        print(f"soak on {self.net or 'the net'}: privacy service {self.pv}, one package every {a.slot} s "
              f"for {a.seconds} s on core {a.core}; best {start_head['best']}, finalized {start_head['finalized']}", flush=True)
        while time.time() - t_start < a.seconds:
            # a package overdue: its root never came, so everything after it is stale too
            overdue = [t for t in self.txs if t.get("landed") is None and not t.get("dropped")
                       and time.time() - t["submitted"] > a.drop_after]
            if overdue:
                for t in self.txs:
                    if t.get("landed") is None and not t.get("dropped") and t["submitted"] >= overdue[0]["submitted"]:
                        t["dropped"] = True; self.log("dropped", k=t["k"], package=t["package"][:16])
                self.resync(f"package {overdue[0]['k']} not on chain after {a.drop_after} s")
                next_at = time.time()
                continue
            wait = next_at - time.time()
            if wait > 0:
                time.sleep(min(wait, 1)); continue
            try:
                item = self.ready.get(timeout=a.slot)
            except queue.Empty:
                self.log("starved"); continue
            if item["epoch"] != self.epoch:
                continue                         # proven before a resync: its old root is gone
            tx = self.send(item)
            if tx is None:
                next_at += a.slot; continue
            next_at = max(next_at + a.slot, time.time())
            if time.time() - last_report >= 60:
                self.progress(t_start); last_report = time.time()
        # the last packages: give them the same time as any other to land
        deadline = time.time() + a.drop_after
        while time.time() < deadline and any(t.get("landed") is None and not t.get("dropped") for t in self.txs):
            time.sleep(1)
        for t in self.txs:
            if t.get("landed") is None and not t.get("dropped"):
                t["dropped"] = True; self.log("dropped", k=t["k"], package=t["package"][:16], at_end=True)
        self.stop.set(); time.sleep(a.poll + 1)
        return self.report(t_start, start_head)

    def status(self, r):
        try:
            st = http("GET", f"/status?package={r['package_hash']}&anchor={r['anchor']}").get("status") or {}
        except Exception:  # noqa: BLE001 — unknown is not reported
            return "unknown"
        return next(iter(st), "unknown") if isinstance(st, dict) else str(st)

    def send(self, item):
        """Submit, then wait until Reported (or landed); resend on silence or Failed."""
        a = self.a
        tx = {"k": len(self.txs), "submitted": time.time(), "old": item["old"], "new": item["new"],
              "prove_s": item["prove_s"], "landed": None, "sends": 0}
        self.txs.append(tx); self.by_root[item["new"]] = tx
        while tx["sends"] < a.max_sends and not self.stop.is_set():
            try:
                r = http("POST", f"/submit/{self.pv}?core={a.core}", item["payload"], timeout=120)
            except Exception as e:  # noqa: BLE001
                self.log("submit_error", k=tx["k"], error=str(e)[:200]); time.sleep(a.slot); tx["sends"] += 1; continue
            tx["sends"] += 1; tx["package"] = r.get("package_hash", ""); sent = time.time()
            self.log("submit", k=tx["k"], package=tx["package"][:16], anchor_slot=r.get("anchor_slot"), send=tx["sends"])
            while time.time() - sent < a.report_after:
                if tx.get("landed"):
                    return tx
                s = self.status(r)
                if s in ("Reported", "Ready"):
                    tx["reported_s"] = round(time.time() - tx["submitted"], 2)
                    return tx
                if s == "Failed":
                    self.log("failed", k=tx["k"]); break
                time.sleep(1)
            self.log("resend", k=tx["k"], after_s=round(time.time() - sent, 1))
        return tx

    def progress(self, t_start):
        landed = [t for t in self.txs if t.get("landed")]
        lat = [t["latency_s"] for t in landed]
        hd = self.heads[-1] if self.heads else (0, 0, 0)
        print(f"  {time.time() - t_start:5.0f} s: submitted {len(self.txs)}, landed {len(landed)}, "
              f"dropped {sum(1 for t in self.txs if t.get('dropped'))}, latency p50 "
              f"{statistics.median(lat) if lat else 0:.1f} s, best {hd[1]}, finalized {hd[2]}", flush=True)

    def report(self, t_start, start_head):
        a = self.a
        landed = [t for t in self.txs if t.get("landed")]
        dropped = [t for t in self.txs if t.get("dropped")]
        lat = sorted(t["latency_s"] for t in landed)
        q = lambda p: lat[min(len(lat) - 1, int(p * len(lat)))] if lat else None  # noqa: E731
        fin = [h[2] for h in self.heads]
        # the longest stretch with finality not moving
        stall, last_change = 0.0, self.heads[0][0] if self.heads else t_start
        for (t0, _, f0), (t1, _, f1) in zip(self.heads, self.heads[1:]):
            if f1 != f0:
                last_change = t1
            stall = max(stall, t1 - last_change)
        end_head = http("GET", "/head")
        events = [json.loads(l) for l in (self.out / "events.jsonl").read_text().splitlines()]
        prove_errors = sum(1 for e in events if e["kind"] == "prove_error")
        starved = sum(1 for e in events if e["kind"] == "starved")
        pace = len(self.txs) / max(1.0, a.seconds / a.slot)          # over the sending window, not the final wait
        verdict = (not dropped and len(landed) == len(self.txs) and len(self.txs) > 0
                   and prove_errors == 0 and starved == 0 and pace >= 0.9
                   and end_head["finalized"] > start_head["finalized"] and stall <= a.max_stall)
        rep = {"net": self.net, "seconds": round(time.time() - t_start), "slot_s": a.slot, "core": a.core,
               "submitted": len(self.txs), "landed": len(landed), "dropped": len(dropped),
               "latency_s": {"p50": q(0.5), "p90": q(0.9), "p99": q(0.99), "max": lat[-1] if lat else None},
               "prove_s_p50": round(statistics.median([t["prove_s"] for t in self.txs]), 3) if self.txs else None,
               "packages_per_minute": round(len(landed) / max(1, (time.time() - t_start) / 60), 2),
               "resends": sum(max(0, t.get("sends", 1) - 1) for t in self.txs),
               "prove_errors": prove_errors, "starved_slots": starved, "pace_vs_one_a_slot": round(pace, 3),
               "best": [start_head["best"], end_head["best"]],
               "finalized": [start_head["finalized"], end_head["finalized"]],
               "finality_longest_stall_s": round(stall, 1),
               "verdict": "PASS" if verdict else "FAIL"}
        (self.out / "report.json").write_text(json.dumps(rep, indent=1))
        md = ["| measure | value |", "|---|---|"] + [f"| {k} | {v} |" for k, v in rep.items()]
        (self.out / "report.md").write_text("\n".join(md) + "\n")
        print("\n".join(md), flush=True)
        return 0 if verdict else 1


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("--seconds", type=int, default=3600)
    p.add_argument("--slot", type=float, default=6.0)
    p.add_argument("--core", type=int, default=0)
    p.add_argument("--ahead", type=int, default=4)
    p.add_argument("--poll", type=float, default=2.0)
    p.add_argument("--drop-after", type=float, default=120.0)
    p.add_argument("--report-after", type=float, default=30.0)
    p.add_argument("--max-sends", type=int, default=3)
    p.add_argument("--max-stall", type=float, default=60.0)
    p.add_argument("--out", default=str(HERE.parent / "out" / "soak"))
    sys.exit(Soak(p.parse_args()).run())
