# Tools

Everything the build installs, with the install line and the version pinned.

| Tool | Version | Install |
| --- | --- | --- |
| jam080-build | jamswap `tools/jam080` | `cargo build --release --manifest-path ../jamswap/tools/jam080/builder/Cargo.toml`; the GP 0.8.0 blob builder (polkavm-linker 0.36) with the patched `jam-pvm-common` 0.1.28 as a path dependency. `jam-pvm-build` 0.1.28 from crates.io targets GP 0.7.2 and its blobs do not run on lasair 2.x |
| nargo (Noir) | 1.0.0-rc.5 | `noirup -v 1.0.0-rc.5`, then copy `~/.nargo/bin/nargo` to `~/.nargo/versions/nargo-1.0.0-rc.5` (the client calls that path; `NARGO=` overrides). The spike also keeps `nargo-1.0.0-beta.16` there for its bb 3 comparison |
| bb (Barretenberg) | 5.0.0 | `bbup -v 5.0.0`, then copy `~/.bb/bb` to `~/.bb/versions/bb-5.0.0` (`BB=` overrides). The version Noir rc.5 pins. The verifier is zkVerify's PR #9 pinned in abutlabs/ultrahonk_verifier `bb-v5` (a4b2a4af5d); `bb-3.0.3` is kept for the spike's comparison |
| rustc, cargo | 1.92.0 | rustup, with target `riscv64imac-unknown-none-elf` |
| node | 26.11.1 | nvm |
| yarn | 1.22.22 | npm |
| python3 | 3.11.6 | system |
| puppeteer-core | 25.16.0 | `npm install` in `client/e2e` (no browser download; drives the installed Chrome) |
| Google Chrome | 155.0.8059.85 | system, `/Applications/Google Chrome.app`; `CHROME=` overrides |
| openssl | system | `nets/demo.sh` draws mint nonces with `openssl rand -hex 32` |
