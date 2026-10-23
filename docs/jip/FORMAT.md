# How JIPs are written and proposed (#32)

Read from [polkadot-fellows/JIPs](https://github.com/polkadot-fellows/JIPs) at 5202db1
(2026-10-21), its README, its five merged JIPs and its pull requests.

## What a JIP is

A JAM Improvement Proposal is a Markdown file `JIP-N.md` at the repository's root, listed in
`README.md` as `- [JIP-N](./JIP-N.md): **Title** - one-sentence summary.` Five are merged:
JIP-1 (a debug-message host call), JIP-2 (node RPC), JIP-3 (telemetry), JIP-4 (chain spec
file), JIP-5 (secret key derivation). They standardize things the Gray Paper leaves to
implementers or extends beside it. None changes a Gray Paper rule directly; JIP-1 adds a host
call at an index outside the GP's range.

## How one is proposed

A pull request to polkadot-fellows/JIPs that adds the file and its README line, titled
`JIP-N: Title`. Observed practice:
- The number is the next free one, claimed by the PR. JIP-6 (IO tracing, #13, reopened
  elsewhere) and JIP-7 (PVM ecalli trace, #14, open) are in flight, so a new one would be
  JIP-8 unless that is taken first.
- The PR body says what it proposes and links a rendered copy and any implementation or
  examples (#14 links a TypeScript parser and example traces).
- Discussion happens on the PR. Amendments to merged JIPs are PRs too (#17 updates JIP-1, 2
  and 4 for GP 0.8.0).
- No licence header or front matter is used in the files; the repository has no template
  file and no stated review process beyond PR review.

## How a host-call JIP is laid out (JIP-1)

```
# JIP-N: Title

One-paragraph summary.

## Host-call specification

**Index**: <number>             (JIP-1 uses 100, outside the GP's 0..27)
**Name**: `<name>`
**Gas usage**: <expression>
**Input registers**: φ7..       (memory as μ ranges, in the GP's notation)
**Output registers**: φ'7..      (the GP's result codes, e.g. WHAT, OOB)

### Side-effects
<what changes, and what happens on invalid memory>

## Suggestions & examples
```

The GP's notation is used throughout (φ for registers, μ for memory, result codes such as
`WHAT`), so a host-call JIP reads as an extension of GP Appendix B. JIP-1 makes its call
return `WHAT` (the GP's code for an unknown host call), so a service behaves the same whether
or not a node implements it. A proposal whose host calls do work cannot do that; it must say
how a service detects support.

## What this repository's draft follows

`docs/jip/jip-draft.md` uses JIP-1's layout for each host call, the GP's notation, and a
summary line ready for the README. It adds the sections a protocol-cost change needs and
JIP-1 did not (motivation with measurements, gas pricing, determinism, compatibility, test
vectors), as sections after the specification.
