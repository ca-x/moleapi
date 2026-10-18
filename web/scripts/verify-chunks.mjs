import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "@babel/parser";

const assets = fileURLToPath(new URL("../dist/assets/", import.meta.url));
const files = fs.readdirSync(assets).filter((file) => file.endsWith(".js"));
if (!files.length) throw new Error("Frontend build emitted no JavaScript chunks");

const graph = new Map();
for (const file of files) {
  const source = fs.readFileSync(path.join(assets, file), "utf8");
  const program = parse(source, { sourceType: "module" }).program;
  const imports = [];
  for (const node of program.body) {
    if (
      node.type !== "ImportDeclaration" &&
      node.type !== "ExportNamedDeclaration" &&
      node.type !== "ExportAllDeclaration"
    ) continue;
    const target = node.source?.value;
    if (typeof target === "string" && target.startsWith("./")) {
      const dependency = path.basename(target);
      if (dependency.endsWith(".js")) imports.push(dependency);
    }
  }
  graph.set(file, imports);
}

const states = new Map();
const stack = [];
function visit(file) {
  if (states.get(file) === 1) {
    throw new Error(`Frontend chunks have a static import cycle: ${[...stack.slice(stack.indexOf(file)), file].join(" → ")}`);
  }
  if (states.get(file) === 2) return;
  states.set(file, 1);
  stack.push(file);
  for (const dependency of graph.get(file) ?? []) {
    if (!graph.has(dependency)) throw new Error(`${file} imports missing chunk ${dependency}`);
    visit(dependency);
  }
  stack.pop();
  states.set(file, 2);
}
for (const file of files) visit(file);
console.log(`Verified ${files.length} frontend chunks without static import cycles`);
