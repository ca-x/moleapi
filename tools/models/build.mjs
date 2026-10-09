import {build} from 'esbuild';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {catalog} from './entry.js';
const output = new URL('../../vendor/model-engine/', import.meta.url);
await mkdir(output,{recursive:true});
const built = await build({entryPoints:[new URL('./entry.js',import.meta.url).pathname],outfile:new URL('engine.js',output).pathname,bundle:true,platform:'browser',format:'iife',target:'es2022',minify:true,legalComments:'external',metafile:true});
const bytes = await readFile(new URL('engine.js',output));
await writeFile(new URL('manifest.json',output),JSON.stringify({engine:'quicktype-core',version:'26.0.0',schema_converter:'@openapi-contrib/openapi-schema-to-json-schema@5.1.0',sha256:createHash('sha256').update(bytes).digest('hex'),targets:catalog},null,2)+'\n');
const roots = new Set(Object.keys(built.metafile.inputs).filter(path => path.includes('/node_modules/')).map(path => {
  const parts = path.split('/node_modules/').at(-1).split('/');
  return parts[0].startsWith('@') ? parts.slice(0,2).join('/') : parts[0];
}));
for (const name of roots) {
  let license;
  const files = name === 'quicktype-core' ? ['../../quicktype-core-LICENSE'] : ['LICENSE','LICENSE.md','LICENSE-MIT','License','LICENSE.txt','LICENSE-MIT.txt'];
  for (const file of files) {
    try {license = await readFile(new URL(`./node_modules/${name}/${file}`,import.meta.url));break;} catch {}
  }
  if (!license) throw new Error(`Missing license: ${name}`);
  await writeFile(new URL(name.replaceAll('/','-').replace('@','')+'-LICENSE',output),license);
}
