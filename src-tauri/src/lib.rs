//! dsh-desktop: a Tauri shell that hosts the DeepSeek Harness web server
//! (`dsh web`) and points an embedded WebView2 at it.
//!
//! Security model: the harness page (http://127.0.0.1:<port>) is a plain
//! remote page and is never granted Tauri IPC access — `dangerousRemoteDomainIpcAccess`
//! is not enabled. All shell actions go through the native menu/tray and the
//! local boot page.

mod menu;
mod server;

use std::sync::atomic::{AtomicBool, Ordering};
use std::fs;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tauri::menu::CheckMenuItem;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, RunEvent, Size,
    State, WindowEvent, Wry,
};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt as _, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;

/// Ctrl+Alt+D (Cmd+Alt+D on macOS) — chosen to avoid the much more commonly
/// bound Ctrl+Space (IME switching) and Alt+Space (Windows' own system
/// menu). Summons the window from anywhere, same as a tray left-click.
fn global_show_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyD)
}

/// Pulls a usable workspace folder out of launch args — shared by a fresh
/// launch's own `std::env::args()` and the args a second launch attempt
/// hands to `tauri_plugin_single_instance`'s relaunch callback below. Skips
/// the first arg (the exe path itself); returns the first remaining arg
/// that resolves to an existing directory. This is the shape a Windows
/// Explorer "open with" shell command entry passes
/// (`"...\dsh-desktop.exe" "%V"`) — see server.rs's `DSH_DESKTOP_CWD` doc
/// comment for how it's consumed once resolved.
fn requested_workspace(args: &[String]) -> Option<std::path::PathBuf> {
    args.iter().skip(1).map(std::path::PathBuf::from).find(|p| p.is_dir())
}

use server::{DshServer, ServerStatus};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct WindowState {
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    maximized: bool,
}

fn window_state_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join("window.json"))
}

fn restore_window_state(window: &tauri::WebviewWindow) {
    let Some(path) = window_state_path(&window.app_handle()) else { return };
    let Ok(content) = fs::read_to_string(path) else { return };
    let Ok(state) = serde_json::from_str::<WindowState>(&content) else { return };
    if state.width >= 960 && state.height >= 640 {
        let _ = window.set_size(Size::Physical(PhysicalSize { width: state.width, height: state.height }));
    }
    let _ = window.set_position(Position::Physical(PhysicalPosition { x: state.x, y: state.y }));
    if state.maximized { let _ = window.maximize(); }
}

fn save_window_state(window: &tauri::Window) {
    let Some(path) = window_state_path(&window.app_handle()) else { return };
    let Ok(size) = window.inner_size() else { return };
    let Ok(position) = window.outer_position() else { return };
    let state = WindowState {
        width: size.width,
        height: size.height,
        x: position.x,
        y: position.y,
        maximized: window.is_maximized().unwrap_or(false),
    };
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    if let Ok(content) = serde_json::to_string(&state) { let _ = fs::write(path, content); }
}

pub struct AppState {
    pub server: Arc<Mutex<DshServer>>,
    /// Whether the one-time "minimized to tray" notice has fired this run.
    hide_notice_shown: AtomicBool,
    /// The tray's "开机自动启动" checkbox — kept here so
    /// `MENU_TOGGLE_AUTOSTART` can sync its visual state after toggling
    /// (clicking a `CheckMenuItem` doesn't flip its own display automatically).
    autostart_item: CheckMenuItem<Wry>,
}

// ── commands (called only from the local boot page) ─────────────────────────

#[tauri::command]
fn get_status(state: State<'_, AppState>) -> ServerStatus {
    state.server.lock().unwrap().status.clone()
}

#[tauri::command]
fn get_info(app: AppHandle, state: State<'_, AppState>) -> serde_json::Value {
    server::info(&app, &state.server)
}

#[tauri::command]
fn get_connection_settings(app: AppHandle) -> server::ConnectionSettings {
    server::load_settings(&app)
}

#[tauri::command]
fn save_connection_settings(
    app: AppHandle,
    server_url: Option<String>,
    connection_mode: String,
) -> Result<server::ConnectionSettings, String> {
    let mode = connection_mode.trim().to_ascii_lowercase();
    if mode != "smart" && mode != "connect" {
        return Err("连接模式只能是 smart 或 connect。".to_string());
    }
    let normalized_url = server_url
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| {
            if value.starts_with("http://") || value.starts_with("https://") {
                value
            } else {
                format!("http://{value}")
            }
        });
    if mode == "connect"
        && !normalized_url
            .as_deref()
            .is_some_and(|value| value.starts_with("http://") || value.starts_with("https://"))
    {
        return Err("固定地址模式需要有效的 Web UI 地址（http:// 或 https://）。".to_string());
    }
    let settings = server::ConnectionSettings {
        server_url: normalized_url,
        connection_mode: mode,
        ..server::load_settings(&app)
    };
    server::save_settings(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
fn start_server(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let srv = state.server.clone();
    thread::spawn(move || {
        let _ = server::start(&app, &srv);
    });
    Ok(())
}

#[tauri::command]
fn restart_server(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let srv = state.server.clone();
    thread::spawn(move || server::restart(&app, &srv));
    Ok(())
}

#[tauri::command]
fn stop_server(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    server::stop(&app, &state.server);
    server::set_stopped(&app, &state.server);
    Ok(())
}

#[tauri::command]
fn get_log_tail(state: State<'_, AppState>, n: Option<usize>) -> Vec<String> {
    server::log_tail(&state.server, n.unwrap_or(100))
}

#[tauri::command]
fn open_in_browser(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let url = server::running_url(&state.server)
        .unwrap_or_else(|| format!("http://127.0.0.1:{}", server::default_port()));
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_data_dir(app: AppHandle) -> Result<(), String> {
    let home = server::dsh_home_dir(&app);
    let _ = std::fs::create_dir_all(&home);
    app.opener().reveal_item_in_dir(&home).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_autostart_enabled(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// Lets the in-window hamburger menu (Windows/Linux have no native menu bar —
/// see `set_menu()`'s macOS-only gate in `run()`) fire the exact same actions
/// as the tray menu, through the one dispatcher both already share.
#[tauri::command]
fn trigger_menu_action(app: AppHandle, id: String) {
    handle_menu_action(&app, &id);
}

#[derive(serde::Serialize)]
struct UpdateInfo {
    version: String,
    body: Option<String>,
}

/// Checks the configured updater endpoint for a newer shell release. Returns
/// `None` if the current version is already the latest (or the check
/// failed — network errors here shouldn't block the app from starting).
#[tauri::command]
async fn check_for_update(app: AppHandle) -> Option<UpdateInfo> {
    let update = app.updater().ok()?.check().await.ok()??;
    Some(UpdateInfo {
        version: update.version,
        body: update.body,
    })
}

/// Re-checks for an update and, if one is still available, downloads,
/// verifies (against the pinned pubkey) and installs it, then relaunches.
/// Re-checking here (rather than trusting a version string round-tripped
/// from `check_for_update`) avoids installing a stale/unverified `Update`.
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "没有可用的更新".to_string())?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}

// ── window helpers ───────────────────────────────────────────────────────────

/// Un-hide, un-minimize, and focus the main window. Used by both the tray's
/// "显示窗口" action and the single-instance relaunch callback below, so a
/// second launch attempt (desktop icon, Start menu, ...) surfaces the
/// existing window instead of spawning a second process/window/tray icon.
/// Callers include `tauri_plugin_single_instance`'s relaunch callback, which
/// fires on whatever thread delivers the second-instance IPC message — not
/// necessarily the main/event-loop thread window operations need to run on.
/// Confirmed by direct testing: called from that path, `win.show()` et al.
/// silently no-op (previously `let _ = ...`'d away with nothing logged) even
/// though the exact same window responds fine to an externally-driven
/// activation. `run_on_main_thread` dispatches back to the right thread
/// regardless of where this was called from, so every caller — tray click,
/// single-instance relaunch — gets the same, actually-working behavior.
fn show_main_window(app: &AppHandle) {
    let app_in_closure = app.clone();
    let result = app.run_on_main_thread(move || {
        let app = app_in_closure;
        if let Some(win) = app.get_webview_window("main") {
            if let Err(e) = win.show() {
                eprintln!("[dsh-desktop] show_main_window: show() failed: {e}");
            }
            if let Err(e) = win.unminimize() {
                eprintln!("[dsh-desktop] show_main_window: unminimize() failed: {e}");
            }
            if let Err(e) = win.set_focus() {
                eprintln!("[dsh-desktop] show_main_window: set_focus() failed: {e}");
            }
        } else {
            eprintln!("[dsh-desktop] show_main_window: no window labeled \"main\"");
        }
    });
    if let Err(e) = result {
        eprintln!("[dsh-desktop] show_main_window: run_on_main_thread failed: {e}");
    }
}

/// Single-instance relaunch path (second launch attempt while the app is
/// already running). Unlike the tray/hotkey callers, this is *not* a user
/// summoning the window on purpose — it fires for any duplicate launch,
/// including one this app didn't ask for (a stale launcher, a file
/// association, an autostart job). Showing + focusing unconditionally then
/// yanks the window to the front and steals focus from whatever the user is
/// doing in another app.
///
/// So: only summon the window when it isn't already on screen. If it is
/// visible and not minimized, leave it completely alone. If it was hidden
/// to the tray or minimized, surface it normally — that's what a user who
/// re-launches the app wants. Same thread discipline as `show_main_window`
/// (window APIs must run on the main/event-loop thread).
fn show_main_window_on_relaunch(app: &AppHandle) {
    let app_in_closure = app.clone();
    let result = app.run_on_main_thread(move || {
        let app = app_in_closure;
        if let Some(win) = app.get_webview_window("main") {
            match (win.is_visible(), win.is_minimized()) {
                (Ok(true), Ok(false)) => {
                    // Already on screen — do nothing, don't steal focus.
                }
                _ => show_main_window(&app),
            }
        } else {
            eprintln!("[dsh-desktop] show_main_window_on_relaunch: no window labeled \"main\"");
        }
    });
    if let Err(e) = result {
        eprintln!("[dsh-desktop] show_main_window_on_relaunch: run_on_main_thread failed: {e}");
    }
}

/// Disables WebView2's built-in right-click context menu (Back / Forward /
/// Reload / Inspect / …). The harness page is a plain remote page with no
/// Tauri-side navigation UI of its own, so that default menu was the only
/// way a user could reach browser-style back/forward — and "back" lands on
/// the local boot page with no way back to the harness UI short of a
/// restart (there is no in-app forward affordance, and the boot page's own
/// readiness check does not re-run on a history navigation). Removing the
/// menu removes the discoverable path into that dead end. `Settings` lives
/// on the `CoreWebView2` instance, not the page, so this only needs to run
/// once — it stays in effect across every later `navigate()` call (both to
/// the harness URL and back to the boot page on stop/restart).
#[cfg(windows)]
/// Removes only the context-menu items that caused real problems — not the
/// whole menu. An earlier version called
/// `SetAreDefaultContextMenusEnabled(false)`, which does disable the
/// back/forward navigation-trap this exists to prevent, but that's the same
/// menu Copy/Cut/Paste/Select All live on — nuking it took basic clipboard
/// interaction out with it. `ContextMenuRequested` lets the menu items be
/// inspected and selectively removed before it's shown, so "back"/"forward"
/// (the navigation trap) and "reload"/"inspectElement" (would reload the
/// remote harness page out from under its own client-side state, and
/// re-open the DevTools access `disable_devtools` closes below) go, while
/// everything else — Copy chief among them — stays.
fn disable_context_menu(win: &tauri::WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2Controller, ICoreWebView2_11};
    use windows::core::{Interface, PWSTR};
    let _ = win.with_webview(|webview| {
        let controller: ICoreWebView2Controller = webview.controller();
        let result: windows::core::Result<()> = (|| unsafe {
            let core = controller.CoreWebView2()?;
            let core11: ICoreWebView2_11 = core.cast()?;
            let mut token = Default::default();
            core11.add_ContextMenuRequested(
                &webview2_com::ContextMenuRequestedEventHandler::create(Box::new(
                    move |_sender, args| {
                        let Some(args) = args else { return Ok(()) };
                        let items = args.MenuItems()?;
                        let mut count = 0u32;
                        items.Count(&mut count)?;
                        // Remove back-to-front: RemoveValueAtIndex shifts
                        // every later index down by one, so removing
                        // forward-to-back would skip whatever lands on an
                        // already-visited index.
                        for i in (0..count).rev() {
                            let item = items.GetValueAtIndex(i)?;
                            let mut name_ptr = PWSTR::null();
                            item.Name(&mut name_ptr)?;
                            let name = name_ptr.to_string().unwrap_or_default();
                            if matches!(name.as_str(), "back" | "forward" | "reload" | "inspectElement") {
                                items.RemoveValueAtIndex(i)?;
                            }
                        }
                        Ok(())
                    },
                )),
                &mut token,
            )?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("[dsh-desktop] failed to trim WebView2 context menu: {e}");
        }
    });
}
#[cfg(not(windows))]
fn disable_context_menu(_win: &tauri::WebviewWindow) {}

#[cfg(windows)]
/// Embedded WebView2 controls (no visible browser chrome to show a
/// permission prompt) default every permission-gated Web API — clipboard
/// included — to Deny unless the host explicitly grants it here. The
/// iframed harness page's own message Copy button calls the standard
/// `navigator.clipboard.writeText()`; under that default it rejects, and
/// the page's own caller swallows the rejection silently (`if (!ok)
/// return;`), which just looks like the button does nothing (see GitHub
/// issue #18). This is the fix on the host side, not a workaround on the
/// page — the page has no way to know it's running inside a host that
/// never grants what a normal browser tab would. Grants only
/// CLIPBOARD_READ (the WebView2 SDK's single permission kind gating the
/// whole clipboard API surface, both read and write, despite the name) —
/// every other kind (camera, mic, geolocation, notifications, ...) is left
/// unhandled, which resolves to WebView2's own default Deny. Same
/// one-time-at-setup reasoning as `disable_context_menu`: a
/// `CoreWebView2`-level event registration, not a per-page one.
fn allow_clipboard_permission(win: &tauri::WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Controller, COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ,
        COREWEBVIEW2_PERMISSION_STATE_ALLOW,
    };
    let _ = win.with_webview(|webview| {
        let controller: ICoreWebView2Controller = webview.controller();
        let result: windows::core::Result<()> = (|| unsafe {
            let core = controller.CoreWebView2()?;
            let mut token = Default::default();
            core.add_PermissionRequested(
                &webview2_com::PermissionRequestedEventHandler::create(Box::new(
                    move |_sender, args| {
                        let Some(args) = args else { return Ok(()) };
                        let mut kind = Default::default();
                        args.PermissionKind(&mut kind)?;
                        if kind == COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ {
                            args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
                        }
                        Ok(())
                    },
                )),
                &mut token,
            )?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("[dsh-desktop] failed to grant WebView2 clipboard permission: {e}");
        }
    });
}
#[cfg(not(windows))]
fn allow_clipboard_permission(_win: &tauri::WebviewWindow) {}

/// Disables Ctrl+scroll / Ctrl+±/0 page zoom entirely. Nothing in this app's
/// own UI (boot page or tray) exposes a zoom control — it's purely the
/// WebView2 accelerator-key default, which had no upper bound and no way
/// back to 100% short of restarting the app (Ctrl+0 didn't reset it; a
/// zoomed-out-to-nothing or zoomed-in-past-usable state persisted for the
/// rest of the session). Same one-time-at-setup reasoning as
/// `disable_context_menu`: this is a `CoreWebView2` setting, not a per-page
/// one, so it holds across every later `navigate()` call.
#[cfg(windows)]
fn disable_zoom_control(win: &tauri::WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller;
    let _ = win.with_webview(|webview| {
        let controller: ICoreWebView2Controller = webview.controller();
        let result: windows::core::Result<()> = (|| unsafe {
            let core = controller.CoreWebView2()?;
            let settings = core.Settings()?;
            settings.SetIsZoomControlEnabled(false)?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("[dsh-desktop] failed to disable WebView2 zoom control: {e}");
        }
    });
}
#[cfg(not(windows))]
fn disable_zoom_control(_win: &tauri::WebviewWindow) {}

/// Disables WebView2's built-in DevTools (F12 / Ctrl+Shift+I / right-click →
/// 检查) in release builds only — debug builds keep it, since it's the
/// normal way to debug the harness page during development. Release is a
/// consumer-facing build: an unlabeled "检查" entry (now moot, since the
/// context menu is off — but the keyboard shortcuts reach DevTools
/// independently of the context menu) would only confuse a non-technical
/// user who triggers it by accident, and leaving it reachable in a shipped
/// build is needless extra attack surface for no product benefit here.
#[cfg(all(windows, not(debug_assertions)))]
fn disable_devtools(win: &tauri::WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller;
    let _ = win.with_webview(|webview| {
        let controller: ICoreWebView2Controller = webview.controller();
        let result: windows::core::Result<()> = (|| unsafe {
            let core = controller.CoreWebView2()?;
            let settings = core.Settings()?;
            settings.SetAreDevToolsEnabled(false)?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("[dsh-desktop] failed to disable WebView2 DevTools: {e}");
        }
    });
}
#[cfg(not(all(windows, not(debug_assertions))))]
fn disable_devtools(_win: &tauri::WebviewWindow) {}

/// Injects a script into the harness iframe that reads the page's CSS custom
/// properties — the same `--dsw-alias-bg-layer-*` tokens the Electron
/// preload (preload.ts) checks — and posts the computed dark/light state to
/// the shell window. The shell then updates its own toolbar/dock chrome to
/// match the in-page theme without needing Tauri IPC access to the iframe.
/// Uses the same `AddScriptToExecuteOnDocumentCreated` + `postMessage`
/// pattern as `inject_file_mention_bridge` above; the same cross-origin
/// boundary considerations apply.
#[cfg(windows)]
fn inject_theme_bridge(win: &tauri::WebviewWindow) {
    use webview2_com::AddScriptToExecuteOnDocumentCreatedCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller;
    use windows::core::HSTRING;

    const SCRIPT: &str = r##"(function () {
  if (window.top === window) return;
  var TOKENS = ["--dsw-alias-bg-base", "--dsw-alias-bg-layer-1", "--dsw-alias-bg-layer-2", "--dsw-alias-label-primary", "--dsw-alias-label-secondary", "--dsw-alias-state-business-primary"];
  function parseColor(c) {
    c = c.trim();
    var m = null;
    if (c.charAt(0) === '#') {
      var h = c.slice(1);
      if (h.length === 3) h = h[0]+h[0]+h[1]+h[1]+h[2]+h[2];
      if (h.length >= 6) return { r:parseInt(h[0]+h[1],16), g:parseInt(h[2]+h[3],16), b:parseInt(h[4]+h[5],16) };
      return null;
    }
    m = c.match(/^rgba?\(\s*(\d+)\s*[,/]\s*(\d+)\s*[,/]\s*(\d+)/);
    if (m) return { r:+m[1], g:+m[2], b:+m[3] };
    return null;
  }
  function isDark() {
    var style = getComputedStyle(document.documentElement);
    for (var i = 0; i < TOKENS.length; i++) {
      var value = style.getPropertyValue(TOKENS[i]).trim();
      if (!value) continue;
      var parsed = parseColor(value);
      if (parsed) return (0.2126 * parsed.r + 0.7152 * parsed.g + 0.0722 * parsed.b) < 128;
    }
    var bg = parseColor(style.backgroundColor);
    if (bg) return (0.2126 * bg.r + 0.7152 * bg.g + 0.0722 * bg.b) < 128;
    return null;
  }
  function report() {
    var dark = isDark();
    var style = getComputedStyle(document.documentElement);
    var tokens = {};
    for (var i = 0; i < TOKENS.length; i++) {
      var value = style.getPropertyValue(TOKENS[i]).trim();
      if (value) tokens[TOKENS[i]] = value;
    }
    if (dark !== null) window.top.postMessage({ source: "dsh-desktop", type: "theme", dark: dark, tokens: tokens }, "*");
  }
  report();
  var obs = new MutationObserver(function () { report(); });
  obs.observe(document.documentElement, { attributes: true, attributeFilter: ["class", "style"] });
})();"##;

    let _ = win.with_webview(|webview| {
        let controller: ICoreWebView2Controller = webview.controller();
        let result: webview2_com::Result<()> = (|| {
            let core = unsafe { controller.CoreWebView2() }?;
            AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
                Box::new(move |handler| unsafe {
                    core.AddScriptToExecuteOnDocumentCreated(&HSTRING::from(SCRIPT), &handler)
                        .map_err(Into::into)
                }),
                Box::new(|_result, _id| Ok(())),
            )
        })();
        if let Err(e) = result {
            eprintln!("[dsh-desktop] failed to inject theme bridge script: {e}");
        }
    });
}
#[cfg(not(windows))]
fn inject_theme_bridge(_win: &tauri::WebviewWindow) {}

/// Routes every outbound `http(s)` link — `target="_blank"`/`window.open`
/// (`NewWindowRequested`) and plain top-level navigation
/// (`NavigationStarting`) alike — to the system's default browser instead of
/// the embedded WebView2. Without a `NewWindowRequested` handler, WebView2's
/// default popup blocker (`IsDefaultPopupBlockerEnabled`, on by default)
/// silently swallows `target="_blank"` clicks — the harness page renders its
/// message/document links that way, so external links otherwise do nothing
/// at all. Without a `NavigationStarting` handler, a plain (non-`_blank`)
/// outbound link would instead navigate the WebView's top level away from
/// the harness UI, with no in-app way back (see `disable_context_menu`'s
/// doc comment on why "back" is removed from the context menu). Handling
/// both here keeps that decision entirely shell-side — it never depends on
/// or routes through the dsh web server. Same one-time-at-setup reasoning as
/// `disable_context_menu`: `CoreWebView2` event registrations, not per-page
/// ones, so they hold across every later `navigate()` call.
///
/// `127.0.0.1`/`localhost` URLs are left alone in both handlers — that's the
/// harness's own page loading, not an outbound link.
#[cfg(windows)]
fn install_external_link_handlers(app: &AppHandle, win: &tauri::WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller;
    use webview2_com::{take_pwstr, NavigationStartingEventHandler, NewWindowRequestedEventHandler};
    use windows::core::PWSTR;

    fn is_external_http(uri: &str) -> bool {
        (uri.starts_with("http://") || uri.starts_with("https://"))
            && !uri.contains("127.0.0.1")
            && !uri.contains("localhost")
    }

    let app_new_window = app.clone();
    let app_navigation = app.clone();
    let _ = win.with_webview(move |webview| {
        let controller: ICoreWebView2Controller = webview.controller();
        let result: windows::core::Result<()> = (|| unsafe {
            let core = controller.CoreWebView2()?;

            let mut token_new_window = Default::default();
            core.add_NewWindowRequested(
                &NewWindowRequestedEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else { return Ok(()) };
                    let mut uri_ptr = PWSTR::null();
                    args.Uri(&mut uri_ptr)?;
                    let uri = take_pwstr(uri_ptr);
                    if is_external_http(&uri) {
                        let _ = app_new_window.opener().open_url(uri, None::<&str>);
                    }
                    // Handled either way: a bare `SetHandled(false)` still
                    // leaves the popup blocked (WebView2's default), it just
                    // stops this handler from being the reason — so the net
                    // effect without this line is identical to today's
                    // "point-blank clicks do nothing" bug this exists to fix.
                    args.SetHandled(true)?;
                    Ok(())
                })),
                &mut token_new_window,
            )?;

            let mut token_navigation = Default::default();
            core.add_NavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else { return Ok(()) };
                    let mut uri_ptr = PWSTR::null();
                    args.Uri(&mut uri_ptr)?;
                    let uri = take_pwstr(uri_ptr);
                    if is_external_http(&uri) {
                        let _ = app_navigation.opener().open_url(uri, None::<&str>);
                        args.SetCancel(true)?;
                    }
                    Ok(())
                })),
                &mut token_navigation,
            )?;

            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("[dsh-desktop] failed to install external link handlers: {e}");
        }
    });
}
#[cfg(not(windows))]
fn install_external_link_handlers(_app: &AppHandle, _win: &tauri::WebviewWindow) {}

// ── menu / tray actions ──────────────────────────────────────────────────────

fn handle_menu_action(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    match id {
        menu::MENU_OPEN_BROWSER => {
            let url = server::running_url(&state.server)
                .unwrap_or_else(|| format!("http://127.0.0.1:{}", server::default_port()));
            let _ = app.opener().open_url(url, None::<&str>);
        }
        menu::MENU_RESTART => {
            let srv = state.server.clone();
            let app2 = app.clone();
            thread::spawn(move || server::restart(&app2, &srv));
        }
        menu::MENU_STOP => {
            server::stop(app, &state.server);
            server::set_stopped(app, &state.server);
        }
        menu::MENU_OPEN_DATA_DIR => {
            let home = server::dsh_home_dir(app);
            let _ = std::fs::create_dir_all(&home);
            let _ = app.opener().reveal_item_in_dir(&home);
        }
        menu::MENU_CONNECTION_SETTINGS => {
            show_main_window(app);
            let _ = app.emit("open-connection-settings", ());
        }
        menu::MENU_SHOW_WINDOW => show_main_window(app),
        menu::MENU_TOGGLE_AUTOSTART => {
            let autolaunch = app.autolaunch();
            let now_enabled = autolaunch.is_enabled().unwrap_or(false);
            let result = if now_enabled { autolaunch.disable() } else { autolaunch.enable() };
            match result {
                Ok(()) => {
                    let _ = state.autostart_item.set_checked(!now_enabled);
                }
                Err(e) => eprintln!("[dsh-desktop] failed to toggle autostart: {e}"),
            }
        }
        menu::MENU_QUIT => {
            // The spawned server (and any live terminal shell) is torn down
            // in the ExitRequested handler at the end of run() — the single
            // place covering every quit path (app-menu ⌘Q role, tray 退出,
            // updater restart).
            app.exit(0);
        }
        _ => {}
    }
}

// ── app entry ────────────────────────────────────────────────────────────────

pub fn run() {
    // A folder arg on the *first* launch (e.g. from a Windows Explorer
    // "open with" shell command entry) picks the workspace the server
    // starts in — set before anything reads DSH_DESKTOP_CWD. An explicit
    // env var the user already set wins over this, same precedence as every
    // other DSH_DESKTOP_* override.
    if std::env::var("DSH_DESKTOP_CWD").is_err() {
        let args: Vec<String> = std::env::args().collect();
        if let Some(dir) = requested_workspace(&args) {
            std::env::set_var("DSH_DESKTOP_CWD", &dir);
        }
    }

    tauri::Builder::default()
        // Must be the first plugin registered: it needs to claim the
        // single-instance lock before anything else in the builder chain
        // runs. A second launch (desktop icon, Start menu, ...) hits this
        // callback in the *first* process and exits immediately instead of
        // creating its own window/tray icon — see `show_main_window`.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // A folder arg here (second launch — e.g. right-clicking a
            // *different* folder in Explorer while the app is already
            // running) switches the running server to that workspace.
            // Doesn't lose anything: dsh persists sessions per-workspace-
            // path under ~/.dsh/sessions/ regardless of which one the
            // server is currently pointed at, so this is "switch which
            // project's history you're looking at", not "discard unsaved
            // work" — restart() (stop + start) already re-reads
            // DSH_DESKTOP_CWD fresh on every call, so setting it here is
            // all switching the workspace takes.
            if let Some(dir) = requested_workspace(&args) {
                std::env::set_var("DSH_DESKTOP_CWD", &dir);
                let state = app.state::<AppState>();
                let srv = state.server.clone();
                let app2 = app.clone();
                thread::spawn(move || server::restart(&app2, &srv));
                // A workspace switch is a deliberate summon (the user just
                // picked a folder to open), same as the tray click/hotkey —
                // always surface it so they see the switch happen, even if
                // the window was technically "visible" but buried behind
                // other windows or on another virtual desktop.
                show_main_window(app);
            } else {
                // No workspace arg: an untargeted duplicate launch (desktop
                // icon, Start menu, or something this app didn't ask for —
                // a stale launcher, autostart job). Don't steal focus when
                // the window is already on screen — see
                // show_main_window_on_relaunch.
                show_main_window_on_relaunch(app);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed && *shortcut == global_show_shortcut() {
                        show_main_window(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_info,
            get_connection_settings,
            save_connection_settings,
            start_server,
            restart_server,
            stop_server,
            get_log_tail,
            open_in_browser,
            open_data_dir,
            get_autostart_enabled,
            trigger_menu_action,
            check_for_update,
            install_update,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // Persistent log file for the desktop shell itself.
            if let Ok(log_dir) = app.path().app_log_dir() {
                server::init_log_file(log_dir.join("desktop.log"));
            }

            // Built directly rather than via `app.state()` — AppState isn't
            // `manage()`d until below, once `build_tray` has handed back the
            // autostart checkbox it needs to be constructed with.
            let srv = Arc::new(Mutex::new(DshServer::default()));

            // The container page (ui/) hosts the harness in an <iframe> and
            // never navigates its own top level — these WebView2-level
            // settings hold for its whole lifetime, including content
            // rendered inside that iframe (ContextMenuRequested and the
            // zoom/DevTools settings are control-level, not frame-level).
            if let Some(win) = app.get_webview_window("main") {
                restore_window_state(&win);
                disable_context_menu(&win);
                disable_zoom_control(&win);
                disable_devtools(&win);
                allow_clipboard_permission(&win);
                inject_theme_bridge(&win);
                install_external_link_handlers(&handle, &win);
            }

            // The window is intentionally frameless on every platform. Do
            // not restore native decorations on macOS: the shell toolbar owns
            // the drag region, macOS-style traffic lights, and function menu.
            // The native macOS application menu remains available at the top
            // of the screen without adding a title bar to the window.
            #[cfg(target_os = "macos")]
            app.set_menu(menu::build_menu(&handle)?)?;
            let autostart_item = menu::build_tray(&handle, handle_menu_action)?;

            // Best-effort: another app may already hold Ctrl+Alt+D. Not
            // being able to summon the window by hotkey isn't worth failing
            // startup over — the tray icon still works either way.
            if let Err(e) = app.global_shortcut().register(global_show_shortcut()) {
                eprintln!("[dsh-desktop] failed to register global shortcut: {e}");
            }

            app.manage(AppState {
                server: srv.clone(),
                hide_notice_shown: AtomicBool::new(false),
                autostart_item,
            });

            // Auto-start the harness server shortly after the window appears.
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(300));
                let _ = server::start(&handle, &srv);
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::Resized { .. } | WindowEvent::Moved { .. }) {
                save_window_state(window);
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                save_window_state(window);
                // Tray-resident mode: closing the window hides it but leaves
                // the dsh server running in the background. Quitting (⌘Q /
                // tray 退出) tears the server down in the ExitRequested
                // handler and then exits the process.
                api.prevent_close();
                let _ = window.hide();

                let app = window.app_handle();
                let state = app.state::<AppState>();
                if state.hide_notice_shown.swap(true, Ordering::Relaxed) {
                    return;
                }
                let _ = app
                    .notification()
                    .builder()
                    .title("DeepSeek Harness 已转入后台")
                    .body("服务仍在运行；从系统托盘图标可重新打开窗口或退出。")
                    .show();
            }
        })
        .on_menu_event(|app, event| handle_menu_action(app, event.id.as_ref()))
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Tear down the spawned dsh server and any live terminal shell
            // before the process exits — otherwise they're orphaned and
            // keep running/serving. Every quit path (the app-menu ⌘Q role
            // on macOS, the tray "退出" item, updater restarts) funnels into
            // ExitRequested before the event loop ends, so this is the one
            // place it needs to happen.
            if let RunEvent::ExitRequested { .. } = event {
                if let Some(state) = app.try_state::<AppState>() {
                    server::stop(&app, &state.server);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::requested_workspace;

    #[test]
    fn finds_first_existing_dir_after_the_exe_path() {
        let tmp = std::env::temp_dir();
        let args = vec!["dsh-desktop.exe".to_string(), tmp.display().to_string()];
        assert_eq!(requested_workspace(&args), Some(tmp));
    }

    #[test]
    fn ignores_a_nonexistent_path() {
        let args = vec![
            "dsh-desktop.exe".to_string(),
            "Z:\\definitely\\not\\a\\real\\path\\hopefully".to_string(),
        ];
        assert_eq!(requested_workspace(&args), None);
    }

    #[test]
    fn no_args_beyond_the_exe_path_finds_nothing() {
        let args = vec!["dsh-desktop.exe".to_string()];
        assert_eq!(requested_workspace(&args), None);
    }

    #[test]
    fn skips_the_exe_path_itself_even_if_its_directory_exists() {
        // The exe's own directory is a real, existing dir — make sure we
        // never mistake arg[0] (the exe path, not a workspace request) for
        // a workspace request just because its parent happens to exist.
        let tmp = std::env::temp_dir();
        let fake_exe = tmp.join("dsh-desktop.exe");
        let args = vec![fake_exe.display().to_string()];
        assert_eq!(requested_workspace(&args), None);
    }
}
