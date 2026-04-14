fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();

    // Embed the target triple so sidecar.rs can find the Tauri-bundled binary name.
    println!("cargo:rustc-env=NARAECLAW_TARGET_TRIPLE={target}");

    // Create a target-triple-suffixed symlink (unix) or copy (windows) so that
    // tauri_build can validate the externalBin path and cargo tauri build can bundle it.
    //
    // Build order: `cargo build --release` first (produces naraeclaw), then
    // `cargo tauri build` (this build.rs runs, creates the symlink, tauri bundles it).
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let release_dir = std::path::Path::new(&manifest).join("../../../target/release");
    std::fs::create_dir_all(&release_dir).ok();

    let src = release_dir.join("naraeclaw");
    let dst = release_dir.join(format!("naraeclaw-{target}"));

    if src.exists() {
        // Refresh symlink every build so it always points to the latest binary.
        let _ = std::fs::remove_file(&dst);
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink(&src, &dst);
        }
        #[cfg(not(unix))]
        {
            let _ = std::fs::copy(&src, &dst);
        }
    } else if !dst.exists() {
        // Placeholder so tauri_build validation passes before naraeclaw is compiled.
        // Replaced by the real symlink once `cargo build --release` has run.
        std::fs::write(&dst, b"").ok();
    }

    tauri_build::build();
}
