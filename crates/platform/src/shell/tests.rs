use super::*;
use windows::Win32::UI::Shell::{SHGetPathFromIDListEx, GPFIDL_DEFAULT};

#[test]
fn reveal_resolves_exact_file_and_directory_paths_without_command_line_quoting() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("中文 空格,目录");
    std::fs::create_dir(&directory).unwrap();
    let file = directory.join("《鬼屋》 (七版), 资料.pdf");
    std::fs::write(&file, "fixture").unwrap();
    for path in [&directory, &file] {
        let expected = crate::filesystem::validate_local_path(path.to_str().unwrap()).unwrap();
        with_reveal_item(path.to_str().unwrap(), move |item| {
            let mut buffer = vec![0u16; 32768];
            unsafe { SHGetPathFromIDListEx(item, &mut buffer, GPFIDL_DEFAULT).ok()? };
            let length = buffer.iter().position(|c| *c == 0).unwrap();
            assert_eq!(
                std::path::PathBuf::from(String::from_utf16(&buffer[..length]).unwrap()),
                expected
            );
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn missing_and_invalid_paths_never_open_explorer() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("不存在.pdf");
    for path in [
        missing.to_str().unwrap(),
        "relative.txt",
        "https://example.com/",
        "C:\\bad\0name",
    ] {
        assert!(with_reveal_item(path, |_| panic!("must not open Explorer")).is_err());
    }
}
