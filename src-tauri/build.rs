fn main() {
    build_identity();
    let target = std::env::var("TARGET").expect("Cargo 应提供 TARGET");
    let extension = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    for name in ["in-line-mcp", "in-line-cli"] {
        let companion = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("binaries")
            .join(format!("{name}-{target}{extension}"));
        if !companion.exists() {
            std::fs::create_dir_all(companion.parent().unwrap()).expect("无法创建伴随程序目录");
            // Only to let cargo tests compile before bundle preparation. The
            // bundler preparation rejects empty or invalid PE companions.
            std::fs::File::create(&companion).expect("无法创建伴随程序构建占位文件");
        }
    }
    tauri_build::build();
}

fn build_identity() {
    use std::process::Command;
    let git = |args: &[&str]| -> Option<String> {
        let output = Command::new("git").args(args).output().ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8(output.stdout)
            .ok()
            .map(|s| s.trim().to_owned())
    };
    for entry in [
        "src",
        "build.rs",
        "Cargo.toml",
        "Cargo.lock",
        "../package.json",
    ] {
        println!("cargo:rerun-if-changed={entry}");
    }
    for entry in ["HEAD", "index", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--git-path", entry]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"]) {
        if let Some(path) = git(&["rev-parse", "--git-path", &reference]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    let head = git(&["rev-parse", "HEAD"])
        .filter(|s| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()));
    let clean =
        git(&["status", "--porcelain", "--untracked-files=normal"]).is_some_and(|s| s.is_empty());
    let checkout_matches = std::env::var("GITHUB_SHA")
        .ok()
        .is_none_or(|sha| Some(sha) == head);
    println!(
        "cargo:rustc-env=IN_LINE_BUILD_BASELINE={}",
        head.as_deref().unwrap_or("unknown")
    );
    println!(
        "cargo:rustc-env=IN_LINE_BUILD_VERIFIED={}",
        head.is_some() && clean && checkout_matches
    );
    println!("cargo:rustc-env=IN_LINE_BUILD_DIRTY={}", !clean);
}
