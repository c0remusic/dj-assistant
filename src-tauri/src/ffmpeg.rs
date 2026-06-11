use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

/// Extracts the version token from the first line of `ffmpeg -version` output.
/// Input:  "ffmpeg version 7.1 Copyright (c) 2000-2024 ..."
/// Output: "7.1"
pub fn parse_ffmpeg_version(banner: &str) -> Option<String> {
    let first = banner.lines().next()?;
    let after = first.strip_prefix("ffmpeg version ")?;
    Some(after.split_whitespace().next()?.to_string())
}

/// Runs the bundled ffmpeg sidecar with the given args, returning stdout as a String.
pub async fn run_ffmpeg(app: &AppHandle, args: &[&str]) -> Result<String, String> {
    let output = app
        .shell()
        .sidecar("ffmpeg")
        .map_err(|e| format!("sidecar resolve failed: {e}"))?
        .args(args)
        .output()
        .await
        .map_err(|e| format!("ffmpeg exec failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "ffmpeg exited with {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::parse_ffmpeg_version;

    #[test]
    fn parses_standard_banner() {
        let banner =
            "ffmpeg version 7.1 Copyright (c) 2000-2024 the FFmpeg developers\nbuilt with gcc";
        assert_eq!(parse_ffmpeg_version(banner).as_deref(), Some("7.1"));
    }

    #[test]
    fn parses_git_build_banner() {
        let banner = "ffmpeg version n7.1-latest-win64-gpl Copyright (c)";
        assert_eq!(
            parse_ffmpeg_version(banner).as_deref(),
            Some("n7.1-latest-win64-gpl")
        );
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_ffmpeg_version("not ffmpeg output"), None);
    }
}
