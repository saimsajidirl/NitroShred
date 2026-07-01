import { Resvg } from "@resvg/resvg-js";
import { readFileSync, writeFileSync, mkdirSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const svg = readFileSync(join(root, "app/ui/assets/logo.svg"));
const iconsDir = join(root, "app/src-tauri/icons");
const uiDir = join(root, "app/ui");

mkdirSync(iconsDir, { recursive: true });

function render(size) {
  const resvg = new Resvg(svg, {
    fitTo: { mode: "width", value: size },
    background: "transparent",
  });
  return resvg.render().asPng();
}

const sizes = [
  ["32x32.png", 32],
  ["128x128.png", 128],
  ["128x128@2x.png", 256],
  ["icon.png", 512],
];

for (const [name, size] of sizes) {
  writeFileSync(join(iconsDir, name), render(size));
}

writeFileSync(join(uiDir, "favicon.png"), render(32));
writeFileSync(join(uiDir, "favicon-192.png"), render(192));
console.log("Icons generated.");
