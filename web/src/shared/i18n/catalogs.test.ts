import { readFileSync, readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { parse } from "@babel/parser";
import { traverseFast } from "@babel/types";
import { expect, it } from "vitest";
import en from "./en.json";
import zh from "./zh-CN.json";
const english = en as Record<string, string>, chinese = zh as Record<string, string>;
it("ships complete genuine English and Chinese catalogs with identical interpolation parameters", () => {
  expect(Object.keys(english).sort()).toEqual(Object.keys(chinese).sort());
  expect(Object.keys(english).length).toBeGreaterThan(700);
  for (const [key, translation] of Object.entries(english)) {
    expect(translation.trim(), key).not.toBe("");
    expect(translation, key).not.toMatch(/[\u3400-\u9fff]/);
    const variables = (value: string) => [...value.matchAll(/\{\{([^}]+)\}\}/g)].map(match => match[1]).sort();
    // The variable-name illustration deliberately translates its identifier.
    if (key !== "{{变量名}}") expect(variables(translation), key).toEqual(variables(chinese[key]));
  }
});
it("covers every application translation reference and leaves no bare Chinese UI literal", () => {
  const root = fileURLToPath(new URL("../../", import.meta.url));
  const missing: string[] = [], bare: string[] = [];
  function visit(directory: string) {
    for (const name of readdirSync(directory)) {
      const file = join(directory, name);
      if (statSync(file).isDirectory()) { if (name !== "i18n") visit(file); continue; }
      if (!/\.tsx?$/.test(name) || /\.test\./.test(name) || name === "testSetup.ts") continue;
      const ast = parse(readFileSync(file, "utf8"), { sourceType: "module", plugins: ["typescript", "jsx"] });
      const ownedKeys = new Set<object>();
      traverseFast(ast, node => {
        if ((node.type !== "CallExpression" && node.type !== "NewExpression") || node.callee.type !== "Identifier" || !["t", "message", "liveTranslation", "LocalizedError"].includes(node.callee.name)) return;
        const key = node.arguments[0];
        if (key?.type !== "StringLiteral") return;
        ownedKeys.add(key);
        if (!Object.hasOwn(english, key.value) && !Object.hasOwn(english, key.value + "_other")) missing.push(`${file}:${key.value}`);
      });
      traverseFast(ast, node => {
        if (node.type === "StringLiteral" && /[\u3400-\u9fff]/.test(node.value) && !ownedKeys.has(node)) bare.push(`${file}:${node.value}`);
        if (node.type === "JSXText" && /[\u3400-\u9fff]/.test(node.value)) bare.push(`${file}:${node.value}`);
        if (node.type === "TemplateElement" && /[\u3400-\u9fff]/.test(node.value.cooked ?? "")) bare.push(`${file}:${node.value.cooked}`);
      });
    }
  }
  visit(root);
  expect(missing).toEqual([]);
  expect(bare).toEqual([]);
});
