// Extract Microsoft's shipped MIT-licensed Chinese NLS table and match its
// numeric IDs to the original English messages in the same pinned AMD build.
const fs=require('fs'),parser=require('../../web/node_modules/@babel/parser'),traverse=require('../../web/node_modules/@babel/traverse').default;
const root='web/node_modules/monaco-editor';let translated;
traverse(parser.parse(fs.readFileSync(root+'/dev/vs/nls.messages.zh-cn.js','utf8')),{AssignmentExpression(p){if(p.node.left.property?.name==='_VSCODE_NLS_MESSAGES')translated=p.node.right.elements.map(n=>n?.value??null)}});
if(!translated)throw Error('Missing official NLS table');const catalog={};
traverse(parser.parse(fs.readFileSync(root+'/dev/vs/editor/editor.main.js','utf8')),{CallExpression(p){const raw=p.node.callee,callee=raw.type==='SequenceExpression'?raw.expressions.at(-1):raw,name=callee.name||callee.property?.name;if(!/^localize2?$/.test(name||''))return;const [id,message]=p.node.arguments;if(id?.type==='NumericLiteral'&&message?.type==='StringLiteral'&&typeof translated[id.value]==='string'){catalog[message.value]=translated[id.value]}}});
catalog['Trigger Inline Edit']='触发行内编辑';
fs.writeFileSync('web/src/shared/i18n/monaco-zh-CN.json',JSON.stringify(catalog,null,2)+'\n');console.log(`Embedded ${Object.keys(catalog).length} official Monaco NLS messages`);
