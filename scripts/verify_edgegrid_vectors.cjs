// Independent oracle: Postman Runtime EdgeGrid computeHeader implementation.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),url=require('node:url');
const modules=process.env.MOLEAPI_POSTMAN_NODE_MODULES||'/tmp/moleapi-postman-runtime-qa/node_modules';
const edgegrid=require(path.join(modules,'postman-runtime/lib/authorizer/edgegrid'));
const config={access_token:'synthetic-access-token',client_token:'synthetic-client-token',client_secret:'synthetic-client-secret',timestamp:'20240101T12:00:00+0000',nonce:'fixed-nonce',max_body_bytes:131072,headers_to_sign:[]};
const cases=[
 {name:'query-and-nondefault-port',method:'GET',url:'https://example.com:8443/a//b?repeat=a&repeat=%E9%9B%AA&encoded=%2F',body:Buffer.alloc(0)},
 {name:'selected-order-and-whitespace',method:'GET',url:'https://example.com/',headers:[['X-Z','  one  two\tthree  '],['X-A','last'],['X-Empty','']],names:['X-Z','X-Empty','X-A'],body:Buffer.alloc(0)},
 {name:'post-binary',method:'POST',url:'https://example.com/',body:Buffer.from([0,1,255,65])},
 {name:'post-prefix-beyond-limit',method:'POST',url:'https://example.com/',body:Buffer.concat([Buffer.alloc(131072,97),Buffer.from('not-hashed-tail')])},
 {name:'put-does-not-hash',method:'PUT',url:'https://example.com/',body:Buffer.from('body is present')},
 {name:'empty-post',method:'POST',url:'https://example.com/',body:Buffer.alloc(0)},
 {name:'signing-host-override',method:'POST',url:'http://actual.example:8080/resource?q=1',base:'https://signer.example:9443/',body:Buffer.from('request body')},
 {name:'tiny-byte-prefix',method:'POST',url:'https://[2001:db8::1]:8443/resource?x=%E9%9B%AA',max:3,body:Buffer.from('雪\u2603')}
];
const fixtures=cases.map(test=>{
 const c={...config,headers_to_sign:test.names||[],base_url:test.base||'',max_body_bytes:test.max||config.max_body_bytes};
 const headers=Object.fromEntries((test.headers||[]).map(([key,value])=>[key.toLowerCase(),value]));
 const bodyHash=test.method==='POST'&&test.body.length?crypto.createHash('sha256').update(test.body.subarray(0,c.max_body_bytes)).digest('base64'):'';
 const expected=edgegrid.computeHeader({accessToken:c.access_token,clientToken:c.client_token,clientSecret:c.client_secret,nonce:c.nonce,timestamp:c.timestamp,headers,headersToSign:c.headers_to_sign,method:test.method,url:url.parse(test.url),baseURL:test.base?url.parse(test.base).host:undefined,bodyHash});
 return {name:test.name,config:c,method:test.method,url:test.url,headers:test.headers||[],body_base64:test.body.toString('base64'),expected};
});
const file=path.join(__dirname,'../crates/core/tests/fixtures/edgegrid/postman-vectors.json'),data=JSON.stringify(fixtures,null,2)+'\n';
if(process.argv.includes('--write'))fs.writeFileSync(file,data);else{if(fs.readFileSync(file,'utf8')!==data)throw Error('Independent EdgeGrid vectors differ');console.log(`${fixtures.length} Postman EdgeGrid vectors verified`);}
