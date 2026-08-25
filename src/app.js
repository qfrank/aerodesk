// AeroDesk frontend — no bundler; uses the global Tauri v2 API (withGlobalTauri).
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const I18N = {
  en: {
    axTitle: "Accessibility required",
    axBody: "Grant Accessibility in System Settings → Privacy & Security so AeroDesk can paste at the cursor.",
    qrHint: "Scan with the Aerofont app to pair.",
    hostLabel: "Host (LAN IP)",
    save: "Save",
    devicesTitle: "Devices",
    receivedTitle: "Received text",
    connected: "Connected",
    disconnected: "Disconnected",
    noDevices: "No devices connected",
  },
  zh: {
    axTitle: "需要辅助功能授权",
    axBody: "在 系统设置 → 隐私与安全性 → 辅助功能 中授权 AeroDesk,才能在光标处粘贴。",
    qrHint: "用 Aerofont 应用扫码配对。",
    hostLabel: "主机(局域网 IP)",
    save: "保存",
    devicesTitle: "设备",
    receivedTitle: "接收文本",
    connected: "已连接",
    disconnected: "未连接",
    noDevices: "暂无设备连接",
  },
};
const lang = (navigator.language || "en").toLowerCase().startsWith("zh") ? "zh" : "en";
const t = (k) => (I18N[lang][k] ?? I18N.en[k] ?? k);

function applyI18n() {
  document.querySelectorAll("[data-i18n]").forEach((el) => {
    el.textContent = t(el.getAttribute("data-i18n"));
  });
}

const $ = (id) => document.getElementById(id);

function render(info) {
  $("qr").innerHTML = info.qrSvg || "";
  $("url").textContent = info.url;
  $("host").value = info.host;
  $("desk-name").textContent = info.name;
}

let axPoll = null;
async function updateAx() {
  try {
    const ok = await invoke("check_accessibility");
    $("ax-banner").classList.toggle("hidden", ok);
    if (ok && axPoll) {
      clearInterval(axPoll);
      axPoll = null;
    }
    return ok;
  } catch (_) {
    return false;
  }
}

function formatTs(ms) {
  const d = new Date(ms);
  const p = (n, l = 2) => String(n).padStart(l, "0");
  return (
    `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ` +
    `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}.${p(d.getMilliseconds(), 3)}`
  );
}

const devices = new Map();

async function refreshDevices() {
  try {
    const list = await invoke("get_devices");
    devices.clear();
    for (const d of list) devices.set(d.name, { connected: d.connected });
  } catch (_) {}
  renderDevices();
}

function renderDevices() {
  const box = $("devices");
  box.innerHTML = "";
  if (devices.size === 0) {
    const empty = document.createElement("div");
    empty.className = "muted dev-empty";
    empty.textContent = t("noDevices");
    box.appendChild(empty);
    return;
  }
  for (const [name, info] of devices) {
    const row = document.createElement("div");
    row.className = "dev-row";
    const dot = document.createElement("span");
    dot.className = `dot ${info.connected ? "ok" : "err"}`;
    const nm = document.createElement("span");
    nm.className = "dev-name";
    nm.textContent = name;
    const st = document.createElement("span");
    st.className = `dev-state ${info.connected ? "on" : "off"}`;
    st.textContent = info.connected ? t("connected") : t("disconnected");
    row.append(dot, nm, st);
    box.appendChild(row);
  }
}

function appendReceived(name, text, ts) {
  const box = $("received");
  const row = document.createElement("div");
  row.className = "rx-row";
  const meta = document.createElement("div");
  meta.className = "rx-meta";
  const fromEl = document.createElement("span");
  fromEl.className = "rx-from";
  fromEl.textContent = `[${name}]`;
  const tsEl = document.createElement("span");
  tsEl.className = "rx-ts";
  tsEl.textContent = formatTs(ts);
  meta.append(fromEl, tsEl);
  const txEl = document.createElement("div");
  txEl.className = "rx-text";
  txEl.textContent = text;
  row.append(meta, txEl);
  box.appendChild(row);
  while (box.children.length > 200) box.firstChild.remove();
  box.scrollTop = box.scrollHeight;
}

async function main() {
  applyI18n();
  render(await invoke("get_pairing_info"));
  await refreshDevices();

  if (!(await updateAx())) {
    axPoll = setInterval(updateAx, 2000);
  }
  window.addEventListener("focus", updateAx);
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) updateAx();
  });

  $("save-host").addEventListener("click", async () => {
    render(await invoke("set_host", { host: $("host").value }));
  });

  await listen("aerodesk://device-connected", (e) => {
    devices.set(e.payload.name, { connected: true });
    renderDevices();
  });
  await listen("aerodesk://device-disconnected", (e) => {
    if (devices.has(e.payload.name)) {
      devices.get(e.payload.name).connected = false;
      renderDevices();
    }
  });
  await listen("aerodesk://text-received", (e) => {
    appendReceived(e.payload.name, e.payload.text, e.payload.ts);
  });
  await listen("aerodesk://send-error", (e) => {
    console.warn("aerodesk send-error", e.payload);
  });
}

main();