// Links Barretenberg's verifier, compiled for rv64e by ../build.sh into ../out/lib/libchonkbb.a.
fn main() {
    if std::env::var("TARGET").map_or(false, |t| t.contains("polkavm")) {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../out/lib");
        let dir = dir.canonicalize().expect("../out/lib: run ../build.sh first");
        println!("cargo:rerun-if-changed={}", dir.join("libchonkbb.a").display());
        println!("cargo:rustc-link-search=native={}", dir.display());
        println!("cargo:rustc-link-lib=static=chonkbb");
        // bb's Grumpkin MSM through call 105 (../gk105.cpp): callers outside bb's own MSM file reach
        // __wrap_, and __real_ is bb's MSM
        for sym in [
            "_ZN2bb21scalar_multiplication9pippengerINS_5curve8GrumpkinEEENT_7ElementENS_14PolynomialSpanIKNS4_11ScalarFieldEEENSt3__14spanIKNS4_13AffineElementELm18446744073709551615EEEbm",
            "_ZN2bb21scalar_multiplication16pippenger_unsafeINS_5curve8GrumpkinEEENT_7ElementENS_14PolynomialSpanIKNS4_11ScalarFieldEEENSt3__14spanIKNS4_13AffineElementELm18446744073709551615EEEm",
        ] {
            println!("cargo:rustc-link-arg=--wrap={sym}");
        }
    }
}
