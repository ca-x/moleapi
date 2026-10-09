import { build } from 'esbuild';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
const output = new URL('../../vendor/snippet-engine/', import.meta.url);
await mkdir(output, {recursive:true});
await build({entryPoints:[new URL('./entry.js',import.meta.url).pathname],outfile:new URL('engine.js',output).pathname,bundle:true,platform:'browser',format:'iife',target:'es2022',minify:true,legalComments:'external',inject:[new URL('./browser-globals.js',import.meta.url).pathname],alias:{path:new URL('./node_modules/path-browserify/index.js',import.meta.url).pathname,querystring:new URL('./node_modules/querystring-es3/index.js',import.meta.url).pathname}});
const code = await readFile(new URL('engine.js', output));
await writeFile(new URL('manifest.json',output),JSON.stringify({engine:'@scalar/snippetz',version:'0.10.5',polyfills:'core-js@3.49.0',supplemental_engine:'postman-code-generators@2.1.1',collection_sdk:'postman-collection@5.3.1',sha256:createHash('sha256').update(code).digest('hex')},null,2)+'\n');
const licenses = ['@scalar/snippetz', '@scalar/helpers', '@scalar/types', 'core-js', 'js-base64', 'postman-code-generators', 'postman-collection', 'buffer', 'path-browserify', 'string_decoder', 'process', 'url', 'postman-url-encoder', 'querystring-es3'];
for (const dependency of licenses) {
  let license;
  for(const filename of dependency === 'js-base64'?['LICENSE.md']:['LICENSE','LICENSE.md','License.md','LICENSE.txt']){
    try {license=await readFile(new URL(`./node_modules/${dependency}/${filename}`,import.meta.url));break;}catch{}
  }
  if(!license)throw new Error(`Dependency license missing: ${dependency}`);
  await writeFile(new URL(`${dependency.replaceAll('/', '-').replace('@','')}-LICENSE`, output), license);
}
