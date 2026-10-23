# Tools

Everything the build installs, with the install line and the version pinned.

| Tool | Version | Install |
| --- | --- | --- |
| jam080-build | jamswap `tools/jam080` | `cargo build --release --manifest-path ../jamswap/tools/jam080/builder/Cargo.toml`; the GP 0.8.0 blob builder (polkavm-linker 0.36) with the patched `jam-pvm-common` 0.1.28 as a path dependency. `jam-pvm-build` 0.1.28 from crates.io targets GP 0.7.2 and its blobs do not run on lasair 2.x |
| nargo (Noir) | 1.0.0-beta.16 | `noirup -v 1.0.0-beta.16` (installer: `curl -L https://raw.githubusercontent.com/noir-lang/noirup/main/install \| bash`). rc.5 pins bb 5.0.0, whose proof layout the Rust verifier does not yet accept |
| bb (Barretenberg) | 3.0.3 | `bbup -v 3.0.3` (installer from aztec-packages `barretenberg/bbup/install`). The release zkVerify's `ultrahonk-no-std` accepts; 3.0.0-nightly.20251104 (Noir beta.16's pin) and 5.0.0 (rc.5's) both fail its parser |
| rustc, cargo | 1.92.0 | rustup, with target `riscv64imac-unknown-none-elf` |
| node | 26.11.1 | nvm |
| yarn | 1.22.22 | npm |
| python3 | 3.11.6 | system |
