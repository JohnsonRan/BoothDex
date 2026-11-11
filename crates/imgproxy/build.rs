// With the `libavif` feature: compile src/aom.c against the system's libavif (found through pkg-config) and link it.

fn main() {
    println!("cargo::rerun-if-changed=src/aom.c");
    #[cfg(feature = "libavif")]
    {
        // prints the link directives for libavif
        let avif = pkg_config::Config::new().atleast_version("1.0").probe("libavif").expect("the libavif feature needs libavif (>= 1.0) and pkg-config: libavif-dev on Debian");
        cc::Build::new().file("src/aom.c").includes(&avif.include_paths).compile("boothdex_aom");
    }
}
