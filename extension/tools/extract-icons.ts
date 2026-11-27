// Extracts the PNG frames baked into desktop/assets/lotus.ico so the browser
// extensions ship the same icon art instead of a redrawn copy.
// Run once per icon change: bun extension/tools/extract-icons.ts
const src = "desktop/assets/lotus.ico";
// Only sizes the .ico really stores: it has no 96 frame, and shipping a
// 128 file named icon-96.png makes the stores reject the icon map.
const targets = [16, 32, 48, 128];
const outDirs = ["extension/firefox/icons", "extension/chromium/icons"];

const data = new Uint8Array(await Bun.file(src).arrayBuffer());
const view = new DataView(data.buffer);
const count = view.getUint16(4, true);

const frames = new Map<number, Uint8Array>();
for (let i = 0; i < count; i++) {
  const entry = 6 + i * 16;
  const size = view.getUint8(entry) || 256;
  const length = view.getUint32(entry + 8, true);
  const offset = view.getUint32(entry + 12, true);
  frames.set(size, data.subarray(offset, offset + length));
}

for (const size of targets) {
  // Exact frame only, so the file on disk is the size its name promises.
  const png = frames.get(size);
  if (!png) {
    console.warn(`icon-${size}.png skipped: the .ico has no ${size}px frame`);
    continue;
  }
  for (const dir of outDirs) {
    await Bun.write(`${dir}/icon-${size}.png`, png);
  }
  console.log(`icon-${size}.png <= ${size}x${size} frame`);
}
