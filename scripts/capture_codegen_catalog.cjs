// Capture upstream CLI naming/library defaults from the pinned engine, not invented emitters.
const fs=require('node:fs');const path=require('node:path');const cp=require('node:child_process');const crypto=require('node:crypto');
const root=path.resolve(__dirname,'..');const yaml=require(path.join(root,'web/node_modules/yaml'));
const java=process.argv[2];if(!java||!path.isAbsolute(java))throw Error('Explicit Java executable required');
const directory=path.join(root,'vendor/openapi-generator');const manifest=JSON.parse(fs.readFileSync(path.join(directory,'manifest.json'),'utf8'));const jar=path.join(directory,'engine.jar');
if(crypto.createHash('sha256').update(fs.readFileSync(jar)).digest('hex')!==manifest.sha256)throw Error('Engine SHA256 mismatch');
const fields=new Set(['packageName','packageVersion','apiPackage','modelPackage','invokerPackage','artifactId','artifactVersion','library','namespace','npmName','npmVersion','groupId']);
for(const target of manifest.targets){
 const text=cp.execFileSync(java,['-Xmx512m','-jar',jar,'config-help','-g',target.id,'-f','yamlsample'],{encoding:'utf8',timeout:15000,maxBuffer:1024*1024});
 // Upstream yamlsample emits unquoted date format strings that are invalid YAML.
 // Parse only the scalar naming/library fields we expose, using the mature YAML parser.
 const selected=text.split('\n').filter(line=>{const m=/^(?:#\s*)?([A-Za-z][A-Za-z0-9_]*):/.exec(line);return m&&fields.has(m[1]);}).map(line=>line.replace(/^#\s*/, '')).join('\n');
 const defaults=yaml.parse(selected)??{};target.options=Object.fromEntries(Object.entries(defaults).filter(([key])=>fields.has(key))); 
}
fs.writeFileSync(path.join(directory,'manifest.json'),JSON.stringify(manifest,null,2)+'\n');console.log(`Captured ${manifest.targets.length} pinned generator option catalogs`);
