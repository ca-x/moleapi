// Synthetic, public test keys; never operational credentials.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const file=path.join(__dirname,'../crates/core/tests/fixtures/asap/keys.json');
if(!fs.existsSync(file)){
 const keys={};for(const [name,options]of [['RSA',{modulusLength:2048}],['ES256',{namedCurve:'prime256v1'}],['ES384',{namedCurve:'secp384r1'}],['ES512',{namedCurve:'secp521r1'}]]){
  const pair=crypto.generateKeyPairSync(name==='RSA'?'rsa':'ec',options);
  keys[name]={pkcs8:pair.privateKey.export({format:'pem',type:'pkcs8'}),traditional:pair.privateKey.export({format:'pem',type:name==='RSA'?'pkcs1':'sec1'}),public:pair.publicKey.export({format:'pem',type:'spki'}),der_base64:pair.privateKey.export({format:'der',type:'pkcs8'}).toString('base64')};
 }fs.writeFileSync(file,JSON.stringify(keys,null,2)+'\n');
}
console.log('Synthetic ASAP test keys ready');
