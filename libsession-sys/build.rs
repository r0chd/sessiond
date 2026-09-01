#[allow(clippy::expect_used)]
fn main() {
    pkg_config::probe_library("libsession").expect(
        "libsession not found via pkg-config; set PKG_CONFIG_PATH to a built libsession's dev output",
    );
}
