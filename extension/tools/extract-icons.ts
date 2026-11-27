// Extracts the PNG frames baked into desktop/assets/lotus.ico so the browser
// extensions ship the same icon art instead of a redrawn copy.
// Run once per icon change: bun extension/tools/extract-icons.ts
const src = "desktop/assets/lotus.ico";
const targets = [16, 32, 48, 96, 128];
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

for (const dir of outDirs) {
  await Bun.write(`${dir}/.keep`, "");
}

for (const size of targets) {
  // Nearest frame at or above the wanted size, so nothing is ever upscaled.
  const available = [...frames.keys()].sort((a, b) => a - b);
  const pick = available.find((s) => s >= size) ?? available[available.length - 1];
  const png = frames.get(pick);
  if (!png) continue;
  for (const dir of outDirs) {
    await Bun.write(`${dir}/icon-${size}.png`, png);
  }
  console.log(`icon-${size}.png <= ${pick}x${pick} frame`);
}
