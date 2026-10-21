import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../src/", import.meta.url));
const modules = new Map();
function visit(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) { visit(absolute); continue; }
    if (!entry.isFile() || !/\.[cm]?[jt]sx?$/.test(entry.name)) continue;
    const relative = path.relative(root, absolute);
    // Extensionless imports must identify one module on every supported OS.
    const key = relative.replace(/\.[cm]?[jt]sx?$/, "").normalize("NFC").toLowerCase();
    const previous = modules.get(key);
    if (previous) throw new Error(`Source module paths collide on case-insensitive systems: ${previous}, ${relative}`);
    modules.set(key, relative);
  }
}
visit(root);
if (!modules.size) throw new Error("No frontend source modules found");
console.log(`Verified ${modules.size} frontend module paths without case-insensitive collisions`);
