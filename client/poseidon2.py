#!/usr/bin/env python3
"""Poseidon2 over BN254 (t = 4, 8 full and 56 partial rounds, x^5), the permutation Noir's
blackbox solver runs, and the sponge noir-lang/poseidon v0.4.0's `Poseidon2::hash` uses
(rate 3, the input length times 2^64 as the initial capacity). The constants come from
Noir v1.0.0-rc.5's solver (client/poseidon2-bn254.json). Pure Python: the client hashes
in-process instead of running a Noir program per hash; the tests below tie it to both.
"""
import json
from pathlib import Path

P = 21888242871839275222246405745257275088548364400416034343698204186575808495617
_C = json.loads((Path(__file__).resolve().parent / "poseidon2-bn254.json").read_text())
DIAG = [int(x, 16) for x in _C["internal_matrix_diagonal"]]
RC = [[int(x, 16) for x in row] for row in _C["round_constant"]]
RF, RP = _C["rounds_f"], _C["rounds_p"]


def _mds4(s):
    t0 = (s[0] + s[1]) % P; t1 = (s[2] + s[3]) % P
    t2 = (2 * s[1] + t1) % P; t3 = (2 * s[3] + t0) % P
    t4 = (4 * t1 + t3) % P; t5 = (4 * t0 + t2) % P
    return [(t3 + t5) % P, t5, (t2 + t4) % P, t4]


def permute(state):
    s = _mds4([x % P for x in state])
    for r in range(RF // 2):
        s = _mds4([pow((s[i] + RC[r][i]) % P, 5, P) for i in range(4)])
    for r in range(RF // 2, RF // 2 + RP):
        s[0] = pow((s[0] + RC[r][0]) % P, 5, P)
        total = sum(s) % P
        s = [(s[i] * DIAG[i] + total) % P for i in range(4)]
    for r in range(RF // 2 + RP, RF + RP):
        s = _mds4([pow((s[i] + RC[r][i]) % P, 5, P) for i in range(4)])
    return s


def hash(inputs):
    """noir-lang/poseidon `Poseidon2::hash(inputs, inputs.len())`."""
    n = len(inputs)
    state = [0, 0, 0, (n << 64) % P]
    for c in range(n // 3):
        for i in range(3):
            state[i] = (state[i] + inputs[3 * c + i]) % P
        state = permute(state)
    rem = n % 3
    for j in range(rem):
        state[j] = (state[j] + inputs[n - rem + j]) % P
    if n == 0 or rem:
        state = permute(state)
    return state[0]


if __name__ == "__main__":
    for case in _C["permutation_vectors"]:
        assert permute([int(x, 16) for x in case["in"]]) == [int(x, 16) for x in case["out"]], case
    # the circuits' own outputs (circuits/hash_cli, executed by nargo)
    assert hash([7, 11, 13]) == 0x1c7467e229cb2aadc6793686c07a90a4b96f2ee7c6b557e3c6ef5d92f1fa253c
    assert hash([0, 0]) == 0x0b63a53787021a4a962a452c2921b3663aff1ffd8d5510540f8e659e782956f1
    z = 0
    for _ in range(16):
        z = hash([z, z])
    assert z == 0x14f44d672eb357739e42463497f9fdac46623af863eea4d947ca00a497dcdeb3, hex(z)   # the empty tree
    # (depth 40, the trees the service keeps, is client/genesis_check.py's: against lib.rs)
    print(f"poseidon2: {len(_C['permutation_vectors'])} permutation vectors, hash_cli's H(7,11,13) and H(0,0), and the empty depth-16 root all match")
