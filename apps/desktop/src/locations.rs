use std::path::PathBuf;
use tauri::Manager;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanLocation {
    id: &'static str,
    name: &'static str,
    path: String,
    description: &'static str,
}

pub fn available(app: &tauri::AppHandle) -> Vec<ScanLocation> {
    from_paths(
        app.path().download_dir().ok(),
        std::env::var_os("TEMP").map(PathBuf::from),
    )
}

fn from_paths(downloads: Option<PathBuf>, temporary: Option<PathBuf>) -> Vec<ScanLocation> {
    [
        (
            "downloads",
            "查看下载中的大文件",
            downloads,
            "扫描下载文件夹，查找安装包、视频等大文件",
        ),
        (
            "temporary",
            "检查临时文件",
            temporary,
            "只扫描当前用户的临时文件夹",
        ),
    ]
    .into_iter()
    .filter_map(|(id, name, path, description)| {
        let path = path.filter(|path| path.is_dir())?;
        Some(ScanLocation {
            id,
            name,
            path: path.to_string_lossy().into_owned(),
            description,
        })
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_locations_are_not_offered_and_paths_are_not_guessed() {
        assert!(from_paths(None, None).is_empty());
        let actual = std::env::temp_dir();
        let result = from_paths(Some(actual.clone()), None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, actual.to_string_lossy());
        assert_eq!(result[0].id, "downloads");
    }
}
