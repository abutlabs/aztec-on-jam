# Tools

Everything the build installs, with the install line and the version pinned.

| Tool | Version | Install |
| --- | --- | --- |
| jam080-build | jamswap `tools/jam080` | `cargo build --release --manifest-path ../jamswap/tools/jam080/builder/Cargo.toml`; the GP 0.8.0 blob builder (polkavm-linker 0.36) with the patched `jam-pvm-common` 0.1.28 as a path dependency. `jam-pvm-build` 0.1.28 from crates.io targets GP 0.7.2 and its blobs do not run on lasair 2.x |
| nargo (Noir) | 1.0.0-rc.5 | `noirup` (installer: `curl -L https://raw.githubusercontent.com/noir-lang/noirup/main/install \| bash`) |
| bb (Barretenberg) | 5.0.0 | `bbup -v 5.0.0` (installer from aztec-packages `barretenberg/bbup/install`); the version Noir rc.5's `scripts/install_bb.sh` pins |
| rustc, cargo | 1.92.0 | rustup, with target `riscv64imac-unknown-none-elf` |
| node | 26.11.1 | nvm |
| yarn | 1.22.22 | npm |
| python3 | 3.11.6 | system |
