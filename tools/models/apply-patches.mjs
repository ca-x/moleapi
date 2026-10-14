import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

export async function applyModelPatches() {
  const packageRoot = new URL('./node_modules/quicktype-core/', import.meta.url);
  const patchRoot = new URL('../../vendor/model-engine/compat/', import.meta.url);
  const manifest = JSON.parse(await readFile(new URL('patches.json', patchRoot), 'utf8'));
  const installed = JSON.parse(await readFile(new URL('package.json', packageRoot), 'utf8'));
  if (installed.name !== manifest.package || installed.version !== manifest.version) throw Error('Model patch package mismatch');
  const sha = bytes => createHash('sha256').update(bytes).digest('hex');
  for (const item of manifest.patches) {
    const target = new URL(item.file, packageRoot);
    const digest = sha(await readFile(target));
    if (digest === item.patched_sha256) continue;
    if (digest !== item.original_sha256) throw Error('Model patch source mismatch: ' + item.file);
    execFileSync('patch', ['-p1', '-f', '-i', fileURLToPath(new URL(item.patch, patchRoot))], {cwd: fileURLToPath(packageRoot), stdio: 'inherit'});
    if (sha(await readFile(target)) !== item.patched_sha256) throw Error('Model patch result mismatch: ' + item.file);
  }
}
