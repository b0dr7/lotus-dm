// Lotus Download Manager - Firefox (MV2) background page.
const api = typeof browser !== "undefined" ? browser : chrome;

const PORT_KEY = "lotusBridgePort";
const PORT_DEFAULT = 8460;

async function bridgePort() {
  try {
    const stored = await api.storage.local.get(PORT_KEY);
    const port = Number(stored && stored[PORT_KEY]);
    if (Number.isInteger(port) && port >= 1024 && port <= 65535) return port;
  } catch (err) {}
  return PORT_DEFAULT;
}

async function sendToLotus(url, filename) {
  const port = await bridgePort();
  const response = await fetch("http://127.0.0.1:" + port + "/add", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ url: url, filename: filename || null })
  });
  let data = {};
  try {
    data = await response.json();
  } catch (err) {}
  if (!response.ok || data.ok !== true) {
    throw new Error(data.error || "HTTP " + response.status);
  }
  return port;
}

function flashFail() {
  api.browserAction.setBadgeBackgroundColor({ color: "#B4472F" });
  api.browserAction.setBadgeText({ text: "!" });
  setTimeout(() => api.browserAction.setBadgeText({ text: "" }), 4000);
}

function looksLikeFilename(text) {
  if (!text || text.length > 200) return null;
  return /\.[a-z0-9]{2,5}$/i.test(text.trim()) ? text.trim() : null;
}

api.runtime.onInstalled.addListener(() => {
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
});

api.contextMenus.onClicked.addListener(async (info) => {
  const url = info.linkUrl || info.srcUrl || info.pageUrl;
  if (!url || !/^https?:/i.test(url)) return;
  const filename = looksLikeFilename(info.linkText);
  try {
    await sendToLotus(url, filename);
    api.browserAction.setBadgeText({ text: "" });
  } catch (err) {
    flashFail();
  }
});

api.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (!message || message.type !== "lotus-add") return;
  sendToLotus(message.url, message.filename)
    .then((port) => sendResponse({ ok: true, port: port }))
    .catch((err) => sendResponse({ ok: false, error: String(err.message || err) }));
  return true;
});
