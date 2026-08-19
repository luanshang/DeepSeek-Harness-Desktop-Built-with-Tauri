const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

const els = {
  starting: document.getElementById("state-starting"),
  error: document.getElementById("state-error"),
  harnessFrame: document.getElementById("harness-frame"),
  startingDetail: document.getElementById("starting-detail"),
  errorMessage: document.getElementById("error-message"),
  logBox: document.getElementById("log-box"),
  logBoxStarting: document.getElementById("log-box-starting"),
  btnLogsStarting: document.getElementById("btn-logs-starting"),
  btnRetry: document.getElementById("btn-retry"),
  btnRestart: document.getElementById("btn-restart"),
  btnLogs: document.getElementById("btn-logs"),
  btnOpenBrowser: document.getElementById("btn-open-browser"),
  updateBanner: document.getElementById("update-banner"),
  updateText: document.getElementById("update-text"),
  btnUpdateInstall: document.getElementById("btn-update-install"),
  btnUpdateDismiss: document.getElementById("btn-update-dismiss"),
  providerTip: document.getElementById("provider-tip"),
  btnProviderTipDismiss: document.getElementById("btn-provider-tip-dismiss"),
  toolbar: document.getElementById("toolbar"),
  trafficLights: document.getElementById("traffic-lights"),
  btnWinMinimize: document.getElementById("btn-win-minimize"),
  btnWinMaximize: document.getElementById("btn-win-maximize"),
  btnWinClose: document.getElementById("btn-win-close"),
  btnAppMenu: document.getElementById("btn-app-menu"),
  appMenu: document.getElementById("app-menu"),
  menuDismissLayer: document.getElementById("menu-dismiss-layer"),
  connectionSettingsOverlay: document.getElementById("connection-settings-overlay"),
  connectionSettingsClose: document.getElementById("btn-connection-settings-close"),
  connectionSettingsCancel: document.getElementById("btn-connection-settings-cancel"),
  connectionSettingsSave: document.getElementById("btn-connection-settings-save"),
  connectionSettingsUrl: document.getElementById("connection-url-input"),
  connectionSettingsUrlField: document.getElementById("connection-url-field"),
  connectionSettingsError: document.getElementById("connection-settings-error"),
  connectionModeInputs: document.querySelectorAll('input[name="connection-mode"]'),
};

const PROVIDER_TIP_DISMISSED_KEY = "dsh-desktop-provider-tip-dismissed";

function initProviderTip() {
  if (localStorage.getItem(PROVIDER_TIP_DISMISSED_KEY)) return;
  els.providerTip.classList.remove("hidden");
  els.btnProviderTipDismiss.addEventListener("click", () => {
    localStorage.setItem(PROVIDER_TIP_DISMISSED_KEY, "1");
    els.providerTip.classList.add("hidden");
  });
}

let logsVisible = false;
let logsStartingVisible = false;

function show(id) {
  for (const key of ["starting", "error"]) {
    els[key].classList.toggle("hidden", key !== id);
  }
  els.harnessFrame.classList.toggle("hidden", id !== "running");
  document.body.classList.toggle("harness-running", id === "running");
}

async function loadLogsInto(box) {
  try {
    const lines = await invoke("get_log_tail", { n: 200 });
    box.textContent = lines.join("\n");
  } catch (err) {
    box.textContent = `无法读取日志: ${err}`;
  }
}

function toggleLogs() {
  logsVisible = !logsVisible;
  els.logBox.classList.toggle("hidden", !logsVisible);
  els.btnLogs.textContent = logsVisible ? "隐藏日志" : "查看日志";
  if (logsVisible) loadLogsInto(els.logBox);
}

function toggleLogsStarting() {
  logsStartingVisible = !logsStartingVisible;
  els.logBoxStarting.classList.toggle("hidden", !logsStartingVisible);
  els.btnLogsStarting.textContent = logsStartingVisible ? "隐藏日志" : "查看日志";
  if (logsStartingVisible) loadLogsInto(els.logBoxStarting);
}

function render(status) {
  switch (status.state) {
    case "running":
      show("running");
      els.harnessFrame.src = status.url;
      break;
    case "starting":
    case "idle":
      show("starting");
      els.startingDetail.textContent = status.detail || "准备本地服务";
      break;
    case "stopped":
      show("error");
      els.harnessFrame.src = "about:blank";
      els.errorMessage.textContent =
        `服务已停止（exit ${status.code ?? "?"}）。` +
        (status.message ? `\n${status.message}` : "");
      break;
    case "error":
      show("error");
      els.harnessFrame.src = "about:blank";
      els.errorMessage.textContent = status.message || "未知错误";
      break;
    default:
      show("starting");
  }
}

async function refresh() {
  try {
    const status = await invoke("get_status");
    render(status);
  } catch (err) {
    show("error");
    els.errorMessage.textContent = `无法获取状态: ${err}`;
  }
}

// ── app menu ────────────────────────────────────────────────────────

function isAppMenuOpen() {
  return !els.appMenu.classList.contains("hidden");
}

async function openAppMenu() {
  let enabled = false;
  try {
    enabled = await invoke("get_autostart_enabled");
  } catch { /* leave unchecked */ }
  els.appMenu.querySelector(".app-menu-check").classList.toggle("hidden", !enabled);
  els.appMenu.classList.remove("hidden");
  els.menuDismissLayer.classList.remove("hidden");
  els.btnAppMenu.setAttribute("aria-expanded", "true");
}

function closeAppMenu() {
  els.appMenu.classList.add("hidden");
  els.menuDismissLayer.classList.add("hidden");
  els.btnAppMenu.setAttribute("aria-expanded", "false");
}

function initAppMenu() {
  els.menuDismissLayer.addEventListener("click", closeAppMenu);
  els.btnAppMenu.addEventListener("click", (event) => {
    event.stopPropagation();
    if (isAppMenuOpen()) { closeAppMenu(); } else { openAppMenu(); }
  });
  for (const item of els.appMenu.querySelectorAll(".app-menu-item")) {
    item.addEventListener("click", () => {
      const id = item.dataset.menuId;
      closeAppMenu();
      if (id === "connection_settings") {
        openConnectionSettings();
      } else if (id === "restart") {
        show("starting");
        els.startingDetail.textContent = "正在重启本地服务…";
        invoke("restart_server").catch((err) => {
          show("error");
          els.errorMessage.textContent = `重启失败: ${err}`;
        });
      } else {
        invoke("trigger_menu_action", { id }).catch((err) => {
          console.error(`菜单操作失败: ${id}`, err);
        });
      }
    });
  }
  document.addEventListener("click", (event) => {
    if (isAppMenuOpen() && !els.appMenu.contains(event.target)) closeAppMenu();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && isAppMenuOpen()) closeAppMenu();
  });
}

// ── window chrome ───────────────────────────────────────────────────

const appWindow = getCurrentWindow();
function initWindowChrome() {
  // No native titlebar on any platform. The shell always renders its own
  // macOS-style traffic lights and function menu inside the webview.
  els.trafficLights.classList.remove("hidden");
}

function initWindowControls() {
  els.btnWinMinimize.addEventListener("click", () => appWindow.minimize());
  els.btnWinMaximize.addEventListener("click", () => appWindow.toggleMaximize());
  els.btnWinClose.addEventListener("click", () => appWindow.close());
}

function applyPageTheme(dark, tokens = {}) {
  document.body.classList.toggle("page-dark", dark);
  document.body.classList.toggle("page-light", !dark);
  const root = document.documentElement;
  const bg = tokens["--dsw-alias-bg-base"] || (dark ? "#0C121B" : "#F4F8FD");
  const layer = tokens["--dsw-alias-bg-layer-1"] || (dark ? "rgba(17, 26, 39, .55)" : "rgba(255, 255, 255, .55)");
  const layer2 = tokens["--dsw-alias-bg-layer-2"] || (dark ? "rgba(22, 33, 48, .55)" : "rgba(236, 242, 250, .5)");
  const ink = tokens["--dsw-alias-label-primary"] || (dark ? "#EAF2FC" : "#13243E");
  const muted = tokens["--dsw-alias-label-secondary"] || (dark ? "#AFC3DC" : "#40597A");
  const accent = tokens["--dsw-alias-state-business-primary"] || (dark ? "#6E9BE8" : "#3F76D8");
  root.style.setProperty("--aqua-bg", bg);
  root.style.setProperty("--aqua-glass", layer);
  root.style.setProperty("--aqua-glass-2", layer2);
  root.style.setProperty("--aqua-ink", ink);
  root.style.setProperty("--aqua-muted", muted);
  root.style.setProperty("--aqua-accent", accent);
}

// ── connection settings ─────────────────────────────────────────────

function currentConnectionMode() {
  return [...els.connectionModeInputs].find((input) => input.checked)?.value || "smart";
}

function setConnectionMode(mode) {
  for (const input of els.connectionModeInputs) input.checked = input.value === mode;
  els.connectionSettingsUrlField.classList.toggle("hidden", mode !== "connect");
}

async function openConnectionSettings() {
  els.connectionSettingsError.classList.add("hidden");
  try {
    const settings = await invoke("get_connection_settings");
    setConnectionMode(settings.connectionMode || "smart");
    els.connectionSettingsUrl.value = settings.serverUrl || "";
  } catch (err) {
    els.connectionSettingsError.textContent = `无法读取连接设置: ${err}`;
    els.connectionSettingsError.classList.remove("hidden");
  }
  els.connectionSettingsOverlay.classList.remove("hidden");
}

function closeConnectionSettings() {
  els.connectionSettingsOverlay.classList.add("hidden");
}

async function saveConnectionSettings() {
  const mode = currentConnectionMode();
  const serverUrl = els.connectionSettingsUrl.value.trim();
  els.connectionSettingsSave.disabled = true;
  els.connectionSettingsError.classList.add("hidden");
  try {
    await invoke("save_connection_settings", {
      serverUrl: serverUrl || null,
      connectionMode: mode,
    });
    closeConnectionSettings();
    await invoke("restart_server");
  } catch (err) {
    els.connectionSettingsError.textContent = String(err);
    els.connectionSettingsError.classList.remove("hidden");
  } finally {
    els.connectionSettingsSave.disabled = false;
  }
}

function initConnectionSettings() {
  for (const input of els.connectionModeInputs) {
    input.addEventListener("change", () => setConnectionMode(input.value));
  }
  els.connectionSettingsClose.addEventListener("click", closeConnectionSettings);
  els.connectionSettingsCancel.addEventListener("click", closeConnectionSettings);
  els.connectionSettingsSave.addEventListener("click", saveConnectionSettings);
  els.connectionSettingsOverlay.addEventListener("click", (event) => {
    if (event.target === els.connectionSettingsOverlay) closeConnectionSettings();
  });
}

// ── theme from iframe ───────────────────────────────────────────────

window.addEventListener("message", (event) => {
  if (event.source !== els.harnessFrame.contentWindow) return;
  let expectedOrigin = "";
  try {
    expectedOrigin = new URL(els.harnessFrame.src).origin;
  } catch { return; }
  if (event.origin !== expectedOrigin) return;
  const data = event.data;
  if (!data || data.source !== "dsh-desktop") return;
  if (data.type === "theme" && typeof data.dark === "boolean") {
    applyPageTheme(data.dark, data.tokens && typeof data.tokens === "object" ? data.tokens : {});
  }
});

// ── init ────────────────────────────────────────────────────────────

async function checkForUpdate() {
  try {
    const update = await invoke("check_for_update");
    if (!update) return;
    els.updateText.textContent = `发现新版本 ${update.version}`;
    els.updateBanner.classList.remove("hidden");
  } catch { /* best-effort */ }
}

async function init() {
  // The shell uses a frameless custom title bar on every platform.
  // Keep the macOS-style traffic lights and the function menu visible.
  initWindowChrome();
  initWindowControls();
  initAppMenu();
  initConnectionSettings();

  listen("server-status", (event) => render(event.payload));
  listen("open-connection-settings", () => openConnectionSettings());

  els.btnRetry.addEventListener("click", () => {
    els.btnRetry.disabled = true;
    invoke("start_server").catch((err) => {
      els.errorMessage.textContent = `启动失败: ${err}`;
    }).finally(() => { els.btnRetry.disabled = false; });
  });
  els.btnRestart.addEventListener("click", () => {
    els.btnRestart.disabled = true;
    invoke("restart_server").catch((err) => {
      els.errorMessage.textContent = `重启失败: ${err}`;
    }).finally(() => { els.btnRestart.disabled = false; });
  });
  els.btnLogs.addEventListener("click", toggleLogs);
  els.btnLogsStarting.addEventListener("click", toggleLogsStarting);
  els.btnOpenBrowser.addEventListener("click", () => invoke("open_in_browser"));

  els.btnUpdateDismiss.addEventListener("click", () => { els.updateBanner.classList.add("hidden"); });
  els.btnUpdateInstall.addEventListener("click", () => {
    els.btnUpdateInstall.disabled = true;
    els.btnUpdateInstall.textContent = "正在更新…";
    els.btnUpdateDismiss.disabled = true;
    invoke("install_update").catch((err) => {
      els.btnUpdateInstall.disabled = false;
      els.btnUpdateInstall.textContent = "立即更新";
      els.btnUpdateDismiss.disabled = false;
      els.updateText.textContent = `更新失败: ${err}`;
    });
  });

  document.addEventListener("keydown", (e) => {
    if (e.key !== "Escape") return;
    if (!els.connectionSettingsOverlay.classList.contains("hidden")) {
      closeConnectionSettings();
    }
  });

  checkForUpdate();
  initProviderTip();
  await refresh();
}

init();