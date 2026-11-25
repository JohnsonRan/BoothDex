// With the `libavif` feature: compile src/aom.c against the system's libavif (found through pkg-config) and link it.

fn main() {
    println!("cargo::rerun-if-changed=src/aom.c");
    #[cfg(feature = "libavif")]
    {
        // pkg-config's own link directives are not used (cargo_metadata(false)): they would name libavif before the
        // archive below, and the linker, which reads the command line in order and drops a shared library that no
        // unresolved symbol needs yet (`--as-needed`), would then let go of libavif before the archive is read. The
        // calls the archive makes into libavif would have nothing to resolve against. It showed on arm64, whose
        // linker applies the order strictly; the x86_64 build's linker happened to keep libavif.
        let avif = pkg_config::Config::new()
            .atleast_version("1.0")
            .cargo_metadata(false)
            .probe("libavif")
            .expect("the libavif feature needs libavif (>= 1.0) and pkg-config: libavif-dev on Debian");
        cc::Build::new().file("src/aom.c").includes(&avif.include_paths).compile("boothdex_aom");
        // print the link directives after the archive, so rustc puts the archive first
        for path in &avif.link_paths {
            println!("cargo::rustc-link-search=native={}", path.display());
        }
        for lib in &avif.libs {
            println!("cargo::rustc-link-lib={lib}");
        }
    }
}
