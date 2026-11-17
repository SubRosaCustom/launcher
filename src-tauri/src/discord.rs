//! Lets Discord start the game through the launcher: registers the `discord-<appid>` URL scheme
//! and reports whether this process was started by it. Discord keeps the pending join and hands
//! it to the injected client itself, so nothing from the URL is forwarded.

const APPLICATION_ID: u64 = 1546246742461124649;

fn scheme() -> String {
    format!("discord-{APPLICATION_ID}")
}

pub(crate) fn launch_requested() -> bool {
    let prefix = format!("{}:", scheme());
    std::env::args().skip(1).any(|arg| arg.starts_with(&prefix))
}

#[tauri::command]
pub fn discord_launch_requested() -> bool {
    launch_requested()
}

#[cfg(target_os = "windows")]
pub(crate) fn register_launch_handler() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let key = format!("HKCU\\Software\\Classes\\{}", scheme());
    let command = format!("\"{}\" \"%1\"", exe.display());
    let entries: [(String, Vec<&str>); 3] = [
        (key.clone(), vec!["/ve", "/d", "URL:Sub Rosa: Custom"]),
        (key.clone(), vec!["/v", "URL Protocol", "/d", ""]),
        (format!("{key}\\shell\\open\\command"), vec!["/ve", "/d", &command]),
    ];
    for (path, values) in &entries {
        let status = Command::new("reg")
            .args(["add", path])
            .args(values)
            .arg("/f")
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("reg add {path}: {e}"))?;
        if !status.success() {
            return Err(format!("reg add {path} exited with {status}"));
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn register_launch_handler() -> Result<(), String> {
    use std::process::Command;

    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    let dir = std::path::Path::new(&home).join(".local/share/applications");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let file_name = format!("srclauncher-{}.desktop", scheme());
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Sub Rosa: Custom\nExec=\"{}\" %u\nNoDisplay=true\nMimeType=x-scheme-handler/{}\n",
        exe.display(),
        scheme()
    );
    let path = dir.join(&file_name);
    if std::fs::read_to_string(&path).is_ok_and(|current| current == entry) {
        return Ok(());
    }
    std::fs::write(&path, entry).map_err(|e| format!("write {}: {e}", path.display()))?;
    let status = Command::new("xdg-mime")
        .args(["default", &file_name, &format!("x-scheme-handler/{}", scheme())])
        .status()
        .map_err(|e| format!("xdg-mime: {e}"))?;
    if !status.success() {
        return Err(format!("xdg-mime exited with {status}"));
    }
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub(crate) fn register_launch_handler() -> Result<(), String> {
    Ok(())
}
