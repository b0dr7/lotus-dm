const api = chrome;

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

function isMediaSite(url) {
  const lower = String(url || "").trim().toLowerCase();
  return MEDIA_SITES.some((site) => lower.startsWith(site));
}

async function startPort() {
  try {
    const stored = await api.storage.local.get(PORT_KEY);
    const port = Number(stored && stored[PORT_KEY]);
    if (Number.isInteger(port) && port >= 1024 && port <= 65535) return port;
  } catch (err) {}
  return PORT_DEFAULT;
}

function retryable(message) {
  const error = new Error(message);
  error.retry = true;
  return error;
}

async function postAdd(port, body) {
  let response;
  try {
    response = await fetch("http://127.0.0.1:" + port + "/add", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body)
    });
  } catch (err) {
    throw retryable("port " + port + " is closed");
  }
  let data = {};
  try {
    data = await response.json();
  } catch (err) {}
  if (response.ok && data.ok === true) return;
  const error = new Error(data.error || "HTTP " + response.status);
  error.retry = response.status === 404;
  throw error;
}

async function sendToLotus(url, filename, quality) {
  const body = { url: url, filename: filename || null };
  if (quality) body.quality = quality;
  const start = await startPort();
  let last = null;
  for (let offset = 0; offset < PORT_SPAN; offset += 1) {
    const port = start + offset;
    if (port > 65535) break;
    try {
      await postAdd(port, body);
      if (port !== start) await api.storage.local.set({ [PORT_KEY]: port });
      return port;
    } catch (err) {
      last = err;
      if (err.retry !== true) throw err;
    }
  }
  throw last || new Error("no bridge port answered");
}

function defaultQuality(url) {
  return isMediaSite(url) ? "720p" : null;
}

function flashBadge(color, text, delay) {
  api.action.setBadgeBackgroundColor({ color: color });
  api.action.setBadgeText({ text: text });
  setTimeout(() => api.action.setBadgeText({ text: "" }), delay);
}

function flashOk() {
  flashBadge("#3F7F4F", "OK", 3000);
}

function flashFail() {
  flashBadge("#B4472F", "!", 4000);
}

function looksLikeFilename(text) {
  if (!text || text.length > 200) return null;
  return /\.[a-z0-9]{2,5}$/i.test(text.trim()) ? text.trim() : null;
}

async function registerMenus() {
  await api.contextMenus.removeAll();
  api.contextMenus.create({
    id: "lotus-link",
    title: "Download with Lotus DM",
    contexts: ["link", "image", "audio", "video"]
  });
  api.contextMenus.create({
    id: "lotus-page",
    title: "Send this page to Lotus DM",
    contexts: ["page"]
  });
}

api.runtime.onInstalled.addListener(() => {
  registerMenus();
});

api.runtime.onStartup.addListener(() => {
  registerMenus();
});

api.contextMenus.onClicked.addListener(async (info) => {
  const url = info.linkUrl || info.srcUrl || info.pageUrl;
  if (!url || !/^https?:/i.test(url)) return;
  try {
    await sendToLotus(url, looksLikeFilename(info.linkText), defaultQuality(url));
    flashOk();
  } catch (err) {
    flashFail();
  }
});

api.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (!message || message.type !== "lotus-add") return;
  const quality =
    typeof message.quality === "string" && message.quality.length <= 20
      ? message.quality
      : null;
  sendToLotus(message.url, message.filename, quality)
    .then((port) => sendResponse({ ok: true, port: port }))
    .catch((err) => sendResponse({ ok: false, error: String(err.message || err) }));
  return true;
});
