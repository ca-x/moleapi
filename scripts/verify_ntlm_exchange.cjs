// Test-only independent oracle: Postman Runtime's NTLM implementation.
// Binary offsets read fixture security-buffer descriptors; production parsing stays in SSPI.
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),{createRequire}=require('node:module');
const modules=process.env.MOLEAPI_POSTMAN_NODE_MODULES||'/tmp/moleapi-postman-runtime-qa/node_modules';
const source=path.join(modules,'postman-runtime/lib/authorizer/ntlm-message.js');
const moduleObject={exports:{}};
vm.runInNewContext(fs.readFileSync(source,'utf8')+'\nmodule.exports.fixtureResponseKey=(password,user,domain)=>NTOWFv2(create_NT_hashed_password_v1(password),user,domain);module.exports.fixtureMac=hmac_md5;', {require:createRequire(source),module:moduleObject,Buffer,console});
const ntlm=moduleObject.exports;
const fixture=JSON.parse(fs.readFileSync(path.join(__dirname,'../crates/core/tests/fixtures/ntlm/sspi-exchange.json'),'utf8'));
const type2=ntlm.parseType2Message(fixture.challenge,error=>{throw error;});
const type3=Buffer.from(fixture.authenticate.replace(/^NTLM /,''),'base64');
if(type3.readUInt32LE(8)!==3)throw Error('Fixture is not an NTLM authenticate message');
function field(index){const length=type3.readUInt16LE(index),offset=type3.readUInt32LE(index+4);if(offset+length>type3.length)throw Error('Bad fixture descriptor');return type3.subarray(offset,offset+length);}
const domain=field(28).toString('utf16le'),user=field(36).toString('utf16le');
if(domain!==fixture.domain||user!==fixture.username)throw Error('Fixture identity differs');
const ntResponse=field(20);if(ntResponse.length<44||ntResponse[16]!==1||ntResponse[17]!==1)throw Error('Fixture is not NTLMv2');
const key=ntlm.fixtureResponseKey(fixture.password,user,domain);
const expected=ntlm.fixtureMac(key,Buffer.concat([type2.serverChallenge,ntResponse.subarray(16)]));
if(!expected.equals(ntResponse.subarray(0,16)))throw Error('SSPI proof differs from independent Postman NTLMv2 proof');
console.log('SSPI NTLMv2 proof and identity independently verified by Postman Runtime');
