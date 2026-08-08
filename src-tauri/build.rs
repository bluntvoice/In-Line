fn main() {
    let target = std::env::var("TARGET").expect("Cargo 应提供 TARGET");
    let extension = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let sidecar = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("binaries")
        .join(format!("in-line-mcp-{target}{extension}"));
    if !sidecar.exists() {
        std::fs::create_dir_all(sidecar.parent().unwrap()).expect("无法创建 sidecar 目录");
        std::fs::File::create(&sidecar).expect("无法创建 sidecar 构建占位文件");
    }
    tauri_build::build();
}
