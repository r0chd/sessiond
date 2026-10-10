fn main() {
    let library = pkg_config::Config::new()
        .atleast_version("0.1.3")
        .statik(false)
        .cargo_metadata(false)
        .probe("libsession")
        .expect("libsession >= 0.1.3 is required; install its development package and ensure libsession.pc is in PKG_CONFIG_PATH");

    for path in library.link_paths {
        println!("cargo:rustc-link-search=native={}", path.display());
    }
    // Require the shared object even when a static archive is available.
    println!("cargo:rustc-link-lib=dylib:+verbatim=libsession.so");
}
