// Trusted independent fixture: node scripts/verify_hawk_vectors.cjs /path/to/postman-request/lib/hawk
const assert=require('node:assert/strict');const hawk=require(process.argv[2]||'postman-request/lib/hawk');
for(const [algorithm,mac,hash] of [['sha1','fLJOe34SrKxlM+mKIagbj+Og9Yw=','75cUqWHC0LoL3RlSJ4L80N89pSI='],['sha256','EUP58pmxDGI9D78BuWstRQXPFLMHY5Tl6EvaOXxr1IM=','gkAohV8ai/HNrYECPT27qaRmxgsUuVq5zhu1MgLEH0I=']]){
 const header=hawk.header(require('node:url').parse('http://example.com:8000/resource?b=1&a=2'),'POST',{credentials:{id:'client-id',key:'published-fixture-key',algorithm},timestamp:1353832234,nonce:'fixture-nonce',payload:'Hello payload',contentType:'Application/Problem+JSON; charset=utf-8',ext:'extra-data',app:'application-id',dlg:'delegated-id'});
 assert.ok(header.includes('mac="'+mac+'"'));assert.ok(header.includes('hash="'+hash+'"'));console.log(JSON.stringify({algorithm,verified:true}));
}

const escaped=hawk.header(require('node:url').parse('http://example.com:8000/resource?b=1&a=2'),'POST',{credentials:{id:'client-id',key:'published-fixture-key',algorithm:'sha256'},timestamp:1353832234,nonce:'fixture-nonce',payload:'Hello payload',contentType:'Application/Problem+JSON; charset=utf-8',ext:'raw\\value,"quote"',app:'application-id',dlg:'delegated-id'});
assert.ok(escaped.includes('mac="3t/Nl+zEhwszmiWOGKu0ARLRaUvin0BfPqZ9SxLCK+s="'));console.log(JSON.stringify({escaped_extra_data_verified:true}));
