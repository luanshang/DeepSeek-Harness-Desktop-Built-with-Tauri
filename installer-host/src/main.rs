#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
#[cfg(windows)]
use std::os::windows::process::CommandExt as _;
const DETACHED_PROCESS: u32 = 0x00000008;
use tauri::{AppHandle, Emitter, WebviewWindow};

const PRODUCT: &str = "DeepSeek Harness Desktop";
const VERSION: &str = "1.5.0";

// `scripts/build-installer.mjs` replaces this payload before compiling us.
static PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.exe"));

type InstallPath = Arc<Mutex<Option<(PathBuf, String)>>>;

fn install_root_for_mode(mode: &str) -> String {
    if mode == "perMachine" {
        std::env::var("PROGRAMFILES").unwrap_or_else(|_| "C:\\Program Files".to_string())
    } else {
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(local_app_data)
            .join("Programs")
            .to_string_lossy()
            .into_owned()
    }
}

fn installed_exe_path(mode: &str) -> PathBuf {
    PathBuf::from(install_root_for_mode(mode)).join(PRODUCT).join("dsh-desktop.exe")
}

#[tauri::command]
fn installer_info(mode: Option<String>) -> serde_json::Value {
    let mode = mode.as_deref().unwrap_or("currentUser");
    let root = install_root_for_mode(mode);
    serde_json::json!({
        "product": PRODUCT,
        "version": VERSION,
        "defaultInstallPath": format!("{}\\{}", root, PRODUCT),
    })
}

#[tauri::command]
fn browse_install_path() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("选择安装目录")
        .pick_folder()
        .map(|path| path.to_string_lossy().into_owned())
}

#[tauri::command]
async fn install_payload(
    app: AppHandle,
    window: WebviewWindow,
    state: tauri::State<'_, InstallPath>,
    path: String,
    mode: String,
) -> Result<(), String> {
    if mode != "currentUser" && mode != "perMachine" {
        return Err("无效的安装范围".to_string());
    }
    let target = PathBuf::from(path.trim());
    if target.as_os_str().is_empty() {
        return Err("请选择安装目录".to_string());
    }
    fs::create_dir_all(&target).map_err(|e| format!("无法创建安装目录：{e}"))?;
    *state.lock().map_err(|_| "安装状态不可用")? = Some((target.clone(), mode.clone()));

    let temp = std::env::temp_dir().join(format!("dsh-desktop-payload-{}.exe", std::process::id()));
    let mut file = File::create(&temp).map_err(|e| format!("无法准备安装文件：{e}"))?;
    file.write_all(PAYLOAD).map_err(|e| format!("无法写入安装文件：{e}"))?;
    drop(file);

    let emit = |percent: u8, label: &str, detail: &str| {
        let _ = window.emit("installer-progress", serde_json::json!({
            "percent": percent, "label": label, "detail": detail
        }));
    };
    emit(8, "准备安装", "正在检查安装目录…");
    let emit_log = |line: &str| {
        let _ = window.emit("installer-log", serde_json::json!({"line": line}));
    };
    emit_log("正在准备安装文件…");
    emit_log(&format!("安装目录：{}", target.display()));
    emit_log("正在解压安装程序…");

    let mut child = Command::new(&temp)
        .arg("/S")
        .arg(if mode == "perMachine" { "/ALLUSERS" } else { "/CURRENTUSER" })
        .arg(format!("/D={}", target.display()))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动安装程序：{e}"))?;

    let app1 = app.clone();
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = app1.emit("installer-log", serde_json::json!({"line": line}));
            }
        });
    }
    let app2 = app.clone();
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let _ = app2.emit("installer-log", serde_json::json!({"line": line}));
            }
        });
    }

    let status = child.wait().map_err(|e| format!("安装进程错误：{e}"))?;
    let _ = fs::remove_file(&temp);
    if !status.success() {
        emit_log("安装失败：安装程序已退出");
        return Err(format!("安装程序退出，代码：{}", status.code().unwrap_or(-1)));
    }
    emit_log("安装程序已成功完成");
    emit_log("正在更新快捷方式和系统信息…");

    emit(100, "安装完成", "正在准备启动应用…");
    let _ = window.emit("installer-done", ());
    Ok(())
}

#[tauri::command]
fn launch_installed_app(state: tauri::State<'_, InstallPath>, app: AppHandle) -> Result<(), String> {
    let (selected_path, mode) = state.lock().map_err(|_| "安装状态不可用")?.clone()
        .ok_or_else(|| "找不到安装信息".to_string())?;
    let default_exe = installed_exe_path(&mode);
    let selected_exe = selected_path.join("dsh-desktop.exe");
    let exe = [selected_exe, default_exe]
        .into_iter()
        .find(|candidate| candidate.exists())
        .ok_or_else(|| format!("找不到已安装的应用：{}", selected_path.display()))?;
    let mut cmd = Command::new(exe);
    #[cfg(windows)]
    cmd.creation_flags(DETACHED_PROCESS);
    cmd.spawn()
        .map_err(|e| format!("无法启动应用：{e}"))?;
    thread::sleep(Duration::from_millis(500));
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn cancel_installer(app: AppHandle) {
    app.exit(0);
}

fn main() {
    let install_path: InstallPath = Arc::new(Mutex::new(None));
    tauri::Builder::default()
        .manage(install_path)
        .invoke_handler(tauri::generate_handler![
            installer_info,
            browse_install_path,
            install_payload,
            launch_installed_app,
            cancel_installer,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run installer");
}
