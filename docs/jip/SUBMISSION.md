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

- **One client implements all four calls:** lasair (2026-10-27, #56).
  - Tested against this JIP's vectors, go-ethereum v1.14.12's EIP-196/197 and EIP-2537
    vectors, and a second library each.
  - Measured with the services on them: `spikes/jip8-lasair/measure.sh`, *Motivation*'s
    measured column.
  - The privacy demo runs on a lasair net with the calls on, and the same blobs fall back to
    PVM code on PolkaJam.

## Before submitting

1. **Constants from bare metal.** Today's are one shared GitHub VM's p99 (open question 1). Run
   `e2e` on two or more pinned-frequency machines, take the maximum, and choose m.
2. **~~A BLS12-381 baseline as PVM code.~~** Done (#59): `spikes/verifier-profile` kinds 2 and 3;
   as calls, 103 saves 7.4–25.5× and 104 saves 18.7–23.1× (lasair bench, one Mac).
2b. **A second client.** lasair is the only implementation. Another team's, against the same
   vectors, is what turns this from one team's measurement into the protocol's. The client
   work also left the text five questions (*Open questions* 4–8).
3. **The companion GP diff.** Appendix B entries, `definitions.tex` constants, and the Ψ_R and
   Ψ_A dispatch lines (*Process*).
4. **Check the number.** JIP-8 was free at polkadot-fellows/JIPs 5202db1 on 2026-10-21. Rename
   the file to `JIP-8.md` or the next free number, and add the README line.
5. **Real review.** The two reviews in `reviews/` are simulated personas. They found real
   defects, but they are not the people. The PR is where review by client teams and the GP's
   maintainers begins.
6. **The PR.** Title it `JIP-N: Curve host calls for proof verification`. Add this line to the
   repository's README:
   `- [JIP-8](./JIP-8.md): **Curve host calls for proof verification** - BN254, BLS12-381 and Grumpkin multi-scalar multiplication and pairing checks as host calls, with Ethereum's precompile encodings and benchmark-derived gas.`
   The body should lead
   with the reference implementation — the proposal running end to end, Aztec's released stack
   on a six-validator net with the calls — and link the site
   (https://abutlabs.github.io/jam-privacy-service/, its JIP page), a rendered copy of the
   text, this repository's measurements and the vectors. Rename the repository (#72) **before**
   opening the PR: GitHub Pages does not redirect, so links made now would break. The repo is
   public, so check first that nothing in it links lasair's private repository.

## Open work that does not block submission

- #46: re-estimate a direct CHONK check with an amortised IPA (for a later Grumpkin JIP).
- #47: the instruction-level alternative, measured.
- #48: field-wise curve hooks in this project's services, 14% less gas today.
