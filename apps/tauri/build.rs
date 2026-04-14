fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let profile = std::env::var("PROFILE").unwrap_or_default();

    // Embed the target triple so sidecar.rs can find the Tauri-bundled binary name.
    println!("cargo:rustc-env=NARAECLAW_TARGET_TRIPLE={target}");

    // From apps/tauri/, "../../" reaches the repo root, so target/release lives at
    // "../../target/release/naraeclaw". ("../../../" would escape the repo entirely.)
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let release_dir = std::path::Path::new(&manifest).join("../../target/release");
    std::fs::create_dir_all(&release_dir).ok();

    let src = release_dir.join("naraeclaw");
    let dst = release_dir.join(format!("naraeclaw-{target}"));

    if src.exists() {
        // Refresh symlink so it always points to the latest binary.
        let _ = std::fs::remove_file(&dst);
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink(&src, &dst);
        }
        #[cfg(not(unix))]
        {
            let _ = std::fs::copy(&src, &dst);
        }
    } else if profile == "release" {
        // Release / packaging builds must have the real naraeclaw binary.
        // Fail fast — never bundle a zero-byte placeholder into a release artifact.
        panic!(
            "\n\nBuild error: naraeclaw binary not found at {}\n\
             Build order: `cargo build --release` first, then `cargo tauri build`.\n",
            src.display()
        );
    } else {
        // Dev / cargo-check builds: write a placeholder so tauri_build validation
        // passes. This placeholder is only used in development and is NEVER packaged
        // (packaging always uses the release profile, which panics above if missing).
        // Developers must run `cargo build --release` (or set NARAECLAW_BIN) before
        // the sidecar will actually start.
        std::fs::write(&dst, b"").ok();
    }

    tauri_build::build();
}
