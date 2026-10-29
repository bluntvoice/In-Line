//! Read-only launch barrier for an incomplete installer file-group transaction.
pub const MESSAGE: &str = "In Line 安装事务尚未完成；已停止启动。请保留安装目录内的恢复备份，重新运行安装器或联系维护人员恢复。";

pub fn ready_at(directory: &std::path::Path) -> bool {
    matches!(
        directory.join(".in-line-install-transaction").try_exists(),
        Ok(false)
    )
}

pub fn ready() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(ready_at))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_or_unrecognized_transaction_blocks_launch_without_reading_its_contents() {
        let root = std::env::temp_dir().join(format!(
            "inline-install-gate-{}",
            crate::mcp::platform::random_secret().unwrap()
        ));
        std::fs::create_dir(&root).unwrap();
        assert!(ready_at(&root));
        let pending = root.join(".in-line-install-transaction");
        std::fs::create_dir(&pending).unwrap();
        assert!(!ready_at(&root));
        std::fs::remove_dir(&pending).unwrap();
        std::fs::write(&pending, b"not a trusted journal").unwrap();
        assert!(!ready_at(&root));
        std::fs::remove_file(&pending).unwrap();
        assert!(ready_at(&root));
        std::fs::remove_dir(&root).unwrap();
    }
}
