fn main() -> Result<(), pkg_config::Error> {
    let library = pkg_config::Config::new()
        .atleast_version("0.1.3")
        .statik(false)
        .cargo_metadata(false)
        .probe("libsession")?;

    for path in library.link_paths {
        println!("cargo:rustc-link-search=native={}", path.display());
    }

    println!("cargo:rustc-link-lib=dylib:+verbatim=libsession.so");

    Ok(())
}
