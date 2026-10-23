# Ten minutes with jam-privacy-service

> Draft: its claims are checked against the final verification run before this note is removed.

Shielded payments as a JAM service: Aztec's model (notes, nullifiers, client-side proofs)
with JAM doing the checking. This page is for a JAM implementer who wants to see it run,
know what the chain actually does, and decide whether to believe it.

## What happens in one payment

1. **The sender proves, on their own machine** (a browser tab or the Python client), a Noir
   join-split: two notes in, two notes out, an optional unshield to a public account.
   Barretenberg makes an UltraHonk proof (zero-knowledge, Keccak transcript). About a second.
2. **A work-package** carries `[public inputs][proof][two sealed notes]` to the privacy
   service, submitted over JIP-2 (or CE-133).
3. **Refine** verifies the proof on the PVM (a pure-Rust verifier inside the service blob,
   about 169 million gas, under a second), and checks the sealed notes are the ones the
   proof committed to. It reads no state.
4. **Accumulate** checks the proof started from the trees the chain holds (note root,
   nullifier root and size), writes the new roots, the nullifiers in order, the two new leaves
   and their sealed notes. Anything stale or replayed changes nothing.
5. **The receiver** scans the chain's sealed notes with their own key; the one that opens
   and hashes to its leaf is theirs. Nobody told them; nobody else can tell.

Value enters and leaves by **JAM deferred transfers**: a public-balance service burns and
sends a transfer whose memo carries the note commitment (shield); the privacy service sends
one back whose memo names the receiving account (unshield).

## Run it

```sh
scripts/bootstrap.sh          # the pinned toolchain, the sibling checkouts, both services built
nets/demo.sh up               # six lasair validators (or NET=pj6: six PolkaJam), services deployed
nets/demo.sh check            # A shields, pays B, B finds it and unshields; 37 byte-level assertions
open http://localhost:19710/?user=A    # and ?user=B in another browser profile
nets/demo.sh down
```

`check` prints every assertion: three note-root changes, each equal to the prover's and to
the root rebuilt from the chain's own leaves; the nullifiers recorded and the nullifier
root chaining through the three proofs; B's client finding exactly its note by scanning; A
down 100 publicly, B up 40; a replayed send changing nothing.

The browser wallet is self-custodial: its secret never leaves the tab; it publishes its
address (an encryption key and a spend key) on chain, scans for its notes, and proves every
transfer itself with noir_js and bb.js. The server behind it only serves files.

## What to read, in this order

| File | What it shows |
| --- | --- |
| `circuits/transfer/src/main.nr` | the whole privacy rule: membership, nullifiers via an indexed Merkle tree, balance, insertion, the sealed notes' binding |
| `services/privacy/src/lib.rs` | refine (verify) and accumulate (roots, nullifiers, leaves, transfers): about 250 lines |
| `client/privacy.py` | what a prover does: notes, both trees, the witness, the payload |
| `../demo-wallet/jam/lib/core.js` | the same, in a browser |
| `spikes/` | the measurements behind every number on the README |

## Where JAM fits, and where it strains

- **Refine is the right place for the proof.** 169M of a package's 5,000M gas; 29
  proofs fit a full package (5 a tiny one). Accumulate stays cheap (about 100k gas a transfer).
- **One tree takes one package a slot.** Each proof names the old root, so packages on one
  tree must land in order. The demo's builder sends the next package only once the previous
  is reported (a core holds one pending report), which still sustains about one a slot.
  More throughput means more trees (more cores) or folding many transfers into one proof.
- **Clients matter.** On lasair, a package can wait for the receiving guarantor's own
  authoring slot and expire (lasair#93); on PolkaJam the same load loses nothing. The
  numbers and the cause are in the README and the issue.

## What is reduced

The README's *What is real and what is reduced* lists each simplification with its issue.
The largest: the circuit is ours, written to Aztec's shapes, not Aztec's kernel circuits.
