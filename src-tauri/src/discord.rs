//! Lets Discord start the game through the launcher: registers the `discord-<appid>` URL scheme
//! and reports whether this process was started by it. Discord keeps the pending join and hands
//! it to the injected client itself, so nothing from the URL is forwarded.

const APPLICATION_ID: u64 = 1546246742461124649;

fn scheme() -> String {
    format!("discord-{APPLICATION_ID}")
}

pub(crate) fn launch_requested() -> bool {
    launch_requested_in(std::env::args())
}

pub(crate) fn launch_requested_in(args: impl IntoIterator<Item = String>) -> bool {
    let prefix = format!("{}:", scheme());
    args.into_iter().skip(1).any(|arg| arg.starts_with(&prefix))
}

// Discord starts the launcher even when the game is already running; the running client gets the join itself
#[cfg(target_os = "windows")]
pub(crate) fn game_running(executable_name: &str) -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let name = std::path::Path::new(executable_name)
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let output = Command::new("tasklist")
        .args(["/FO", "CSV", "/NH", "/FI", &format!("IMAGENAME eq {name}")])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    match output {
        Ok(out) => String::from_utf8_lossy(&out.stdout)
            .to_lowercase()
            .contains(&format!("\"{name}\"")),
        Err(error) => {
            eprintln!("tasklist failed: {error}");
            false
        }
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn game_running(executable_name: &str) -> bool {
    // comm holds at most 15 bytes of the executable name
    let name: String = std::path::Path::new(executable_name)
        .file_name()
        .map(|n| n.to_string_lossy().chars().take(15).collect())
        .unwrap_or_default();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    entries.flatten().any(|entry| {
        std::fs::read_to_string(entry.path().join("comm")).is_ok_and(|comm| comm.trim_end() == name)
    })
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub(crate) fn game_running(_executable_name: &str) -> bool {
    false
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
