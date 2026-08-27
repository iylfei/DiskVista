pub mod credentials;
pub mod elevated;
pub mod filesystem;
pub mod inventory;
pub mod journal;
pub mod metadata;
pub mod process;
pub mod recycle;
pub mod shell;

pub fn normalize(path: &str) -> String {
    path.trim_start_matches("\\\\?\\")
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

pub fn within(path: &str, root: &str) -> bool {
    let p = normalize(path);
    let r = normalize(root);
    !r.is_empty() && (p == r || p.strip_prefix(&r).is_some_and(|s| s.starts_with('\\')))
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_are_component_aware() {
        assert!(within("C:\\Windows\\System32", "c:\\windows"));
        assert!(!within("C:\\Windows-copy", "C:\\Windows"));
        assert!(within("\\\\?\\C:\\Windows\\a", "C:\\Windows"));
    }
}
