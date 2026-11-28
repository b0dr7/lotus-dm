const api = typeof browser !== "undefined" ? browser : chrome;

const PORT_KEY = "lotusBridgePort";
const PORT_DEFAULT = 8460;
const PORT_SPAN = 10;

const MEDIA_SITES = [
  "https://www.youtube.com/",
  "https://youtube.com/",
  "https://m.youtube.com/",
  "https://music.youtube.com/",
  "https://youtube-nocookie.com/",
  "https://youtu.be/",
  "https://vimeo.com/"
];

const stateEl = document.getElementById("state");
const urlEl = document.getElementById("url");
const sendEl = document.getElementById("send");
const noteEl = document.getElementById("note");
const portEl = document.getElementById("port");
const saveEl = document.getElementById("save");
const qualityRow = document.getElementById("qualityRow");
const qualityEl = document.getElementById("quality");

let currentUrl = "";

function setState(text, cls) {
  stateEl.textContent = text;
  stateEl.className = "state" + (cls ? " " + cls : "");
}

function setNote(text, cls) {
  noteEl.textContent = text;
  noteEl.className = "note" + (cls ? " " + cls : "");
}

async function readPort() {
  try {
    const stored = await api.storage.local.get(PORT_KEY);
    const port = Number(stored && stored[PORT_KEY]);
    if (Number.isInteger(port) && port >= 1024 && port <= 65535) return port;
  } catch (err) {}
  return PORT_DEFAULT;
}

async function pingAt(port) {
  const response = await fetch("http://127.0.0.1:" + port + "/ping", {
    cache: "no-store"
  });
  const data = await response.json();
  if (!data || data.ok !== true) return null;
  return String(data.version || "");
}

async function findBridge() {
  const start = await readPort();
  for (let offset = 0; offset < PORT_SPAN; offset += 1) {
    const port = start + offset;
    if (port > 65535) break;
    try {
      const version = await pingAt(port);
      if (version !== null) return { port: port, version: version };
    } catch (err) {}
  }
  return null;
}

function isMediaPage(url) {
  if (!url) return false;
  const lower = url.trim().toLowerCase();
  if (MEDIA_SITES.some((site) => lower.startsWith(site))) return true;
  return /\.(m3u8|mpd)(\?|#|$)/i.test(lower);
}

function filenameFrom(url) {
  try {
    const path = new URL(url).pathname;
    const last = path.split("/").filter(Boolean).pop() || "";
    return /\.[a-z0-9]{2,5}$/i.test(last) ? last : null;
  } catch (err) {
    return null;
  }
}

async function refresh() {
  portEl.value = String(await readPort());

  const tabs = await api.tabs.query({ active: true, currentWindow: true });
  const tab = tabs && tabs[0];
  const url = tab && tab.url && /^https?:/i.test(tab.url) ? tab.url : "";
  currentUrl = url;
  urlEl.textContent = url || "This page cannot be sent.";
  sendEl.disabled = !url;
  qualityRow.hidden = !isMediaPage(url);

  const bridge = await findBridge();
  setState(bridge ? "connected" : "app not running", bridge ? "on" : "off");
  if (bridge) {
    portEl.value = String(bridge.port);
    setNote(bridge.version ? "Lotus DM " + bridge.version : "", "");
  } else {
    setNote("Open Lotus Download Manager, or set the bridge port below.", "bad");
  }
}

sendEl.addEventListener("click", async () => {
  if (!currentUrl) return;
  const quality = qualityRow.hidden ? null : qualityEl.value;
  sendEl.disabled = true;
  setNote("Sending...", "");
  try {
    const reply = await api.runtime.sendMessage({
      type: "lotus-add",
      url: currentUrl,
      filename: filenameFrom(currentUrl),
      quality: quality
    });
    if (reply && reply.ok) {
      setState("connected", "on");
      setNote("Added to the queue.", "good");
      setTimeout(() => window.close(), 900);
    } else {
      throw new Error((reply && reply.error) || "no answer");
    }
  } catch (err) {
    setState("send failed", "off");
    setNote("Could not reach the app: " + String(err.message || err), "bad");
  } finally {
    sendEl.disabled = false;
  }
});

saveEl.addEventListener("click", async () => {
  const port = Number(portEl.value);
  if (!Number.isInteger(port) || port < 1024 || port > 65535) {
    setNote("Port must be between 1024 and 65535.", "bad");
    return;
  }
  await api.storage.local.set({ [PORT_KEY]: port });
  setNote("Port saved.", "good");
  await refresh();
});

refresh();
