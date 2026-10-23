# JIP-8: submission note for Aodh (#34)

**Nothing has been submitted or posted.** `jip-final.md` is ready to become a pull request to
polkadot-fellows/JIPs when you decide to send it. These are the steps between here and there,
in order.

## What is ready

- `jip-final.md`, laid out as `FORMAT.md` describes from the merged JIPs. It carries a summary,
  a README line, motivation, rationale, the specification in GP notation, pricing, activation,
  determinism, compatibility, test vectors, examples, process, open questions and a change log.
- `vectors/jip8-core.jsonl`, 74 vectors, produced by two implementations a curve that agree.
- Every number reproduces from this repository:
  - **UltraHonk split:** `spikes/verifier-profile/measure.sh` on a PolkaJam testnet.
  - **Constants:** `.github/workflows/jip-e2e.yml` on x86, then
    `spikes/jip-bench/e2e-pricing.py < spikes/jip-bench/ci/e2e-40026200133.txt`.
  - **Vectors:** `spikes/jip-bench` `e2e --vectors`, which writes the same file byte for byte.

## Before submitting

1. **Constants from bare metal.** Today's are one shared GitHub VM's p99 (open question 1). Run
   `e2e` on two or more pinned-frequency machines, take the maximum, and choose m.
2. **A BLS12-381 baseline as PVM code.** BN254's calls have one (7–13×), and BLS12-381's do not.
   `spikes/verifier-profile`'s kind 2 extends to them in the same way.
3. **The companion GP diff.** Appendix B entries, `definitions.tex` constants, and the Ψ_R and
   Ψ_A dispatch lines (*Process*).
4. **Check the number.** JIP-8 was free at polkadot-fellows/JIPs 5202db1 on 2026-10-21. Rename
   the file to `JIP-8.md` or the next free number, and add the README line.
5. **Real review.** The two reviews in `reviews/` are simulated personas. They found real
   defects, but they are not the people. The PR is where review by client teams and the GP's
   maintainers begins.
6. **The PR.** Title it `JIP-N: Curve host calls for proof verification`. The body should link
   a rendered copy, this repository's measurements and the vectors. The repo is public, so
   check first that nothing in it links lasair's private repository.

## Open work that does not block submission

- #46: re-estimate a direct CHONK check with an amortised IPA (for a later Grumpkin JIP).
- #47: the instruction-level alternative, measured.
- #48: field-wise curve hooks in this project's services, 14% less gas today.
