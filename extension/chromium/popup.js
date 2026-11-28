const api = typeof browser !== "undefined" ? browser : chrome;

const PORT_KEY = "lotusBridgePort";
const PORT_DEFAULT = 8460;

const stateEl = document.getElementById("state");
const urlEl = document.getElementById("url");
const sendEl = document.getElementById("send");
const noteEl = document.getElementById("note");
const portEl = document.getElementById("port");
const saveEl = document.getElementById("save");

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

async function ping(port) {
  try {
    const response = await fetch("http:
    const data = await response.json();
    return data && data.ok === true;
  } catch (err) {
    return false;
  }
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
  const port = await readPort();
  portEl.value = String(port);

  const tabs = await api.tabs.query({ active: true, currentWindow: true });
  const tab = tabs && tabs[0];
  const url = tab && tab.url && /^https?:/i.test(tab.url) ? tab.url : "";
  currentUrl = url;
  urlEl.textContent = url || "This page cannot be sent.";
  sendEl.disabled = !url;

  const alive = await ping(port);
  setState(alive ? "connected" : "app not running", alive ? "on" : "off");
  if (!alive) {
    setNote("Open Lotus Download Manager, or check the port below.", "bad");
  }
}

sendEl.addEventListener("click", async () => {
  if (!currentUrl) return;
  sendEl.disabled = true;
  setNote("Sending...", "");
  try {
    const reply = await api.runtime.sendMessage({
      type: "lotus-add",
      url: currentUrl,
      filename: filenameFrom(currentUrl)
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
