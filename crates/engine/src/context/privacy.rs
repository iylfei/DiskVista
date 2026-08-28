use cleaner_platform::within;

fn roots() -> Vec<(String, String)> {
    let mut roots: Vec<_> = ["USERPROFILE", "APPDATA", "LOCALAPPDATA"]
        .into_iter()
        .filter_map(|key| {
            std::env::var(key)
                .ok()
                .filter(|value| !value.is_empty())
                .map(|value| {
                    (
                        key.to_owned(),
                        value.replace('/', "\\").trim_end_matches('\\').to_owned(),
                    )
                })
        })
        .collect();
    roots.sort_by_key(|(_, value)| std::cmp::Reverse(value.chars().count()));
    roots
}

pub fn redact_path(path: &str) -> String {
    redact_path_with(path, &roots())
}

fn redact_path_with(path: &str, roots: &[(String, String)]) -> String {
    let path = path.trim_start_matches("\\\\?\\").replace('/', "\\");
    for (key, root) in roots {
        if within(&path, root) {
            let count = root.trim_end_matches('\\').split('\\').count();
            let suffix = path.split('\\').skip(count).collect::<Vec<_>>().join("\\");
            return if suffix.is_empty() {
                format!("%{key}%")
            } else {
                format!("%{key}%\\{suffix}")
            };
        }
    }
    let parts: Vec<_> = path.split('\\').collect();
    if parts.len() >= 3
        && parts[0].len() == 2
        && parts[0].ends_with(':')
        && parts[1].eq_ignore_ascii_case("Users")
        && !parts[2].is_empty()
    {
        return format!(
            "%USERPROFILE%{}",
            if parts.len() > 3 {
                format!("\\{}", parts[3..].join("\\"))
            } else {
                String::new()
            }
        );
    }
    path
}

fn prefix_length(text: &str, root: &str) -> Option<usize> {
    let mut actual = text.char_indices();
    let mut length = 0;
    for expected in root.chars() {
        let (index, character) = actual.next()?;
        if !((matches!(character, '/' | '\\') && matches!(expected, '/' | '\\'))
            || character.to_lowercase().eq(expected.to_lowercase()))
        {
            return None;
        }
        length = index + character.len_utf8();
    }
    let next = text[length..].chars().next();
    if next.is_some_and(|c| {
        !c.is_whitespace()
            && !matches!(
                c,
                '/' | '\\' | '"' | '\'' | ';' | ':' | ')' | ']' | '}' | '，' | '；' | '。'
            )
    }) {
        return None;
    }
    Some(length)
}

pub fn redact_text(text: &str) -> String {
    redact_text_with(text, &roots())
}

fn redact_text_with(text: &str, roots: &[(String, String)]) -> String {
    let mut output = String::new();
    let mut offset = 0;
    while offset < text.len() {
        let tail = &text[offset..];
        if let Some((key, length)) = roots
            .iter()
            .find_map(|(key, root)| prefix_length(tail, root).map(|length| (key, length)))
        {
            output.push_str(&format!("%{key}%"));
            offset += length;
        } else {
            let c = tail.chars().next().unwrap();
            output.push(c);
            offset += c.len_utf8();
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_respects_components_case_slashes_and_unicode() {
        let roots = vec![("USERPROFILE".into(), "C:\\Users\\测试".into())];
        assert_eq!(
            redact_path_with("\\\\?\\c:\\USERS\\测试\\notes.txt", &roots),
            "%USERPROFILE%\\notes.txt"
        );
        assert_eq!(
            redact_path_with("C:/Users/other/Downloads/a.zip", &roots),
            "%USERPROFILE%\\Downloads\\a.zip"
        );
        assert_eq!(
            redact_text_with(
                "位于 c:/USERS/测试/cache；副本 C:\\Users\\测试-copy",
                &roots
            ),
            "位于 %USERPROFILE%/cache；副本 C:\\Users\\测试-copy"
        );
        assert!(prefix_length("C:\\Users\\测试-copy", &roots[0].1).is_none());
    }
}
