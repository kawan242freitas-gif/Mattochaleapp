import { readFile, mkdir, writeFile, copyFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const projectDir = resolve(scriptDir, "..");
const packageIcons = resolve(projectDir, "..", "node_modules/lucide-react-native/dist/esm/icons");
const outputDir = resolve(projectDir, "assets/lucide");
const iconNames = [
  "house", "search", "shopping-cart", "settings", "wine", "arrow-right",
  "refresh-cw", "chevron-left", "plus", "minus", "copy", "image-plus",
  "upload", "trash", "pencil", "shopping-bag", "star", "calendar-days",
  "chart-no-axes-column-increasing", "bed-double", "brush-cleaning",
  "chevron-right", "chevron-down", "maximize", "x",
];

await mkdir(outputDir, { recursive: true });
for (const name of iconNames) {
  const source = await readFile(resolve(packageIcons, `${name}.mjs`), "utf8");
  const start = source.indexOf("node: [");
  const end = source.indexOf("\n};", start);
  if (start < 0 || end < 0) throw new Error(`Could not read Lucide definition: ${name}`);
  const nodeData = source.slice(start + "node: [".length, end);
  const elements = [];
  const elementPattern = /\[\s*"(path|circle|rect|line|polyline|polygon)",\s*\{([\s\S]*?)\}\s*\]/g;
  for (const match of nodeData.matchAll(elementPattern)) {
    const [, tag, block] = match;
    const attrs = [];
    const attrPattern = /([a-zA-Z]+):\s*(?:"([^"]*)"|([\d.]+))/g;
    for (const attr of block.matchAll(attrPattern)) {
      const [, key, stringValue, numberValue] = attr;
      if (key === "key") continue;
      const value = (stringValue ?? numberValue).replaceAll("&", "&amp;").replaceAll('"', "&quot;");
      attrs.push(`${key}="${value}"`);
    }
    elements.push(`<${tag} ${attrs.join(" ")}/>`);
  }
  if (!elements.length) throw new Error(`No Lucide paths found: ${name}`);
  const svg = `<!-- Lucide ${name}; ISC license, see LICENSE-Lucide.txt -->\n<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${elements.join("")}</svg>\n`;
  await writeFile(resolve(outputDir, `${name}.svg`), svg);
}

await copyFile(resolve(projectDir, "..", "node_modules/lucide-react-native/LICENSE"), resolve(outputDir, "LICENSE-Lucide.txt"));
console.log(`Exported ${iconNames.length} Lucide SVG icons to ${outputDir}`);
