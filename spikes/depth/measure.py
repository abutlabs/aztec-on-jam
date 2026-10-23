#!/usr/bin/env python3
"""What tree depth costs: the transfer circuit at depth D for both trees (note and nullifier),
for D in 16, 24, 32, 40 (Aztec's is 40). For each: ACIR opcodes and circuit size, proving time
(nargo execute + bb prove, a deposit claim), and the refine gas of verifying that proof in the
service on lasair's in-process rig (a service build with that depth's vk embedded).
  python3 spikes/depth/measure.py [16 24 32 40]
"""
import json, os, re, shutil, subprocess, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]; SUB = ROOT.parent
sys.path.insert(0, str(ROOT / "client"))
import privacy  # noqa: E402

BUILD = SUB / "jamswap/tools/jam080/builder/target/release/jam080-build"
DEMO = SUB / "lasair/_build/default/bin/demo_deploy.exe"
FIX = ROOT / "services/privacy/fixtures/transfer.vk"
depths = [int(x) for x in sys.argv[1:]] or [16, 24, 32, 40]
work = Path(tempfile.mkdtemp(prefix="depth-"))
real_vk = FIX.read_bytes()
rows = []
try:
    for d in depths:
        cdir = work / f"d{d}" / "transfer"
        shutil.copytree(ROOT / "circuits/transfer", cdir, ignore=shutil.ignore_patterns("target"))
        src = cdir / "src/main.nr"
        src.write_text(re.sub(r"global (NULL_)?DEPTH: u32 = \d+;", lambda m: f"global {m.group(1) or ''}DEPTH: u32 = {d};", src.read_text()))
        subprocess.run([privacy.NARGO, "compile", "--silence-warnings"], cwd=cdir, check=True, capture_output=True)
        info = subprocess.run([privacy.NARGO, "info"], cwd=cdir, capture_output=True, text=True).stdout
        opcodes = int(re.search(r"\| transfer \| main\s+\|\s+(\d+)", info).group(1))
        # the client at this depth, proving in this copy
        privacy.DEPTH = privacy.NULL_DEPTH = d
        privacy.CIRCUITS = cdir.parent
        privacy._WORK.clear()
        privacy.STATE = work / f"d{d}/state.json"
        privacy.deposit("A", 100)
        t0 = time.time()
        r = privacy.transfer("A", "deposit:0", "-", "A", 100, 0, 0, 0)
        prove_s = time.time() - t0
        vk = privacy.transfer_vk().read_bytes()
        log_n = int.from_bytes(vk[:32], "big")
        FIX.write_bytes(vk)
        out = subprocess.run([str(BUILD), str(ROOT / "services/privacy"), "--out", str(work / f"d{d}")], capture_output=True, text=True).stdout
        jam = out.split()[0]
        rig = subprocess.run([str(DEMO), jam, r["payload"].hex()], cwd=SUB / "lasair", capture_output=True, text=True,
                             env=dict(os.environ, LASAIR_REFINE_GAS="5000000000")).stdout
        verdict = "VALID" if "VALID" in rig and "INVALID" not in rig else "INVALID"
        gas = int(re.search(r"\[3\] REFINE.*?gas used\s*:\s*(\d+)", rig, re.S).group(1))
        rows.append({"depth": d, "acir_opcodes": opcodes, "log_n": log_n, "prove_s": round(prove_s, 2),
                     "refine_gas": gas, "verdict": verdict, "proof_bytes": len(r["payload"]) - 2 - 14 * 32 - 2 - 176})
        print(json.dumps(rows[-1]), flush=True)
finally:
    FIX.write_bytes(real_vk)                       # the service keeps its depth-16 key
    privacy._WORK.clear()
print("| depth | ACIR opcodes | log n | prove (s) | refine gas | verdict |\n|---|---|---|---|---|---|")
for r in rows:
    print(f"| {r['depth']} | {r['acir_opcodes']:,} | {r['log_n']} | {r['prove_s']} | {r['refine_gas']:,} | {r['verdict']} |")
