const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;
const appWindow = getCurrentWindow();

const installer = {
  getInfo: (mode) => invoke("installer_info", { mode }),
  browse: () => invoke("browse_install_path"),
  install: (path, mode) => invoke("install_payload", { path, mode }),
  cancel: () => invoke("cancel_installer"),
  launch: () => invoke("launch_installed_app"),
};
const views = {
  welcome: document.getElementById("view-welcome"),
  options: document.getElementById("view-options"),
  progress: document.getElementById("view-progress"),
  done: document.getElementById("view-done"),
};
const pathInput = document.getElementById("install-path");
const scopeSelect = document.getElementById("install-scope");
const progressBar = document.getElementById("progress-bar");
const progressLabel = document.getElementById("progress-label");
const progressPercent = document.getElementById("progress-percent");
const progressDetail = document.getElementById("progress-detail");
const installLog = document.getElementById("install-log");
const detailBox = document.querySelector(".progress-detail-box");

function show(name) {
  for (const [key, view] of Object.entries(views)) view.classList.toggle("hidden", key !== name);
}
function setProgress(percent, label, detail) {
  const value = Math.max(0, Math.min(100, Number(percent) || 0));
  progressBar.style.width = `${value}%`;
  progressPercent.textContent = `${Math.round(value)}%`;
  progressLabel.textContent = label || "处理中";
  progressDetail.textContent = detail || "请稍候…";
}

async function initialize() {
  const info = await installer.getInfo(scopeSelect?.value || "currentUser");
  document.getElementById("version").textContent = info.version;
  pathInput.value = info.defaultInstallPath;
  scopeSelect.addEventListener("change", async () => {
    const next = await installer.getInfo(scopeSelect.value);
    pathInput.value = next.defaultInstallPath;
  });
}

document.getElementById("btn-close").addEventListener("click", () => installer.cancel());
document.getElementById("drag-region").addEventListener("mousedown", (event) => {
  if (event.button === 0) appWindow.startDragging().catch(() => {});
});
document.getElementById("btn-start").addEventListener("click", () => show("options"));
document.getElementById("btn-back").addEventListener("click", () => show("welcome"));
document.getElementById("btn-browse").addEventListener("click", async () => {
  const selected = await installer.browse();
  if (selected) pathInput.value = selected;
});
document.getElementById("btn-install").addEventListener("click", async () => {
  show("progress");
  setProgress(0, "准备中", "正在检查安装目录…");
  try {
    await installer.install(pathInput.value.trim(), scopeSelect.value);
  } catch (error) {
    show("options");
    alert(String(error));
  }
});
document.getElementById("btn-launch").addEventListener("click", () => {
  installer.launch().catch((err) => alert(String(err)));
});
document.getElementById("btn-finish").addEventListener("click", () => installer.cancel());
listen("installer-progress", (event) => {
  const { percent, label, detail } = event.payload || {};
  setProgress(percent, label, detail);
});
listen("installer-log", (event) => {
  const line = event.payload?.line || "";
  if (!line) return;
  if (installLog) {
    installLog.textContent += line + "\n";
    installLog.scrollTop = installLog.scrollHeight;
  }
});
listen("installer-done", () => show("done"));
initialize().catch((error) => alert(String(error)));
