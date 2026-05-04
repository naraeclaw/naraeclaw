fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();

    // Embed the target triple so sidecar.rs can find the Tauri-bundled binary name.
    println!("cargo:rustc-env=NARAECLAW_TARGET_TRIPLE={target}");

    // The sidecar binary lives in apps/tauri/bin/ with the target triple suffix.
    // Copy from target/release/naraeclaw if available, otherwise check bin/ directly.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let bin_dir = std::path::Path::new(&manifest).join("bin");
    std::fs::create_dir_all(&bin_dir).ok();

    let dst = bin_dir.join(format!("naraeclaw-{target}"));

    // Try to copy from workspace build output.
    let release_src = std::path::Path::new(&manifest).join("../../target/release/naraeclaw");
    if release_src.exists() {
        let _ = std::fs::copy(&release_src, &dst);
    }

    if !dst.exists() {
        eprintln!(
            "cargo:warning=Sidecar binary not found at {}. Run: cargo build --release --bin naraeclaw && cp target/release/naraeclaw apps/tauri/bin/naraeclaw-{target}",
            dst.display()
        );
        // Create empty placeholder for build to proceed.
        std::fs::write(&dst, b"").ok();
    }

    println!("cargo:rerun-if-changed=bin/naraeclaw-{target}");
    tauri_build::build();
}
