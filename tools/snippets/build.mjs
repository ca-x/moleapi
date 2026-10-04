import { build } from 'esbuild';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
const output = new URL('../../vendor/snippet-engine/', import.meta.url);
await mkdir(output, {recursive:true});
await build({entryPoints:[new URL('./entry.js',import.meta.url).pathname],outfile:new URL('engine.js',output).pathname,bundle:true,platform:'browser',format:'iife',target:'es2022',minify:true,legalComments:'external'});
const code = await readFile(new URL('engine.js', output));
await writeFile(new URL('manifest.json',output),JSON.stringify({engine:'@scalar/snippetz',version:'0.10.5',polyfills:'core-js@3.49.0',sha256:createHash('sha256').update(code).digest('hex')},null,2)+'\n');
const licenses = ['@scalar/snippetz', '@scalar/helpers', '@scalar/types', 'core-js', 'js-base64'];
for (const dependency of licenses) {
  const license = await readFile(new URL(`./node_modules/${dependency}/${dependency === "js-base64" ? "LICENSE.md" : "LICENSE"}`, import.meta.url));
  await writeFile(new URL(`${dependency.replaceAll('/', '-').replace('@','')}-LICENSE`, output), license);
}
