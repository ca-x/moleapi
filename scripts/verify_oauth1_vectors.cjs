// Independent oracle: node-oauth1 used by Postman Runtime, not MoleAPI or its Rust SDK.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const root=process.env.MOLEAPI_POSTMAN_NODE_MODULES||'/tmp/moleapi-postman-runtime-qa/node_modules';
const oauth=require(path.join(root,'node-oauth1'));
const directory=path.join(__dirname,'../crates/core/tests/fixtures/oauth1');
const keyPath=path.join(directory,'synthetic-private.pem');
if(!fs.existsSync(keyPath))fs.writeFileSync(keyPath,crypto.generateKeyPairSync('rsa',{modulusLength:2048,privateKeyEncoding:{format:'pem',type:'pkcs8'},publicKeyEncoding:{format:'pem',type:'spki'}}).privateKey);
const privateKey=fs.readFileSync(keyPath,'utf8');
const url='https://example.com:8443/resource?%E9%9B%AA=%26&z=last&%25=one&same=%C3%A9&same=z&empty=&plus=a+b';
const config={consumer_key:'consumer/雪',consumer_secret:'secret&雪',token:'token+value',token_secret:'token/secret',nonce:'nonce雪',timestamp:'1700000000',include_version:true,include_empty_params:true};
const fixtures=[];
for(const algorithm of ['HMAC-SHA1','HMAC-SHA256','HMAC-SHA512','RSA-SHA1','RSA-SHA256','RSA-SHA512','PLAINTEXT'])for(const binary of [false,true]){
 const body=binary?'AAH/QQ==':'same=%2B&%E9%9B%AA=%E2%98%83&empty=&form=yes';
 const parsed=new URL(url),parameters=[...parsed.searchParams];
 if(!binary)parameters.push(...new URLSearchParams(body));
 const fields={oauth_consumer_key:config.consumer_key,oauth_token:config.token,oauth_signature_method:algorithm,oauth_version:'1.0'};
 fields.oauth_nonce=config.nonce;fields.oauth_timestamp=config.timestamp;
 if(binary){const sha=algorithm.endsWith('SHA256')?'sha256':algorithm.endsWith('SHA512')?'sha512':'sha1';fields.oauth_body_hash=crypto.createHash(sha).update(Buffer.from(body,'base64')).digest('base64');}
 parameters.push(...Object.entries(fields));
 const expected=oauth.SignatureMethod.sign({method:'POST',action:parsed.origin+parsed.pathname,parameters},{consumerSecret:config.consumer_secret,tokenSecret:config.token_secret,privateKey});
 fixtures.push({config:{...config,algorithm,private_key:algorithm.startsWith('RSA-')?privateKey:'',include_body_hash:binary},url,body,binary,expected});
}
const serialized=JSON.stringify(fixtures,null,2)+'\n';const fixturePath=path.join(directory,'postman-vectors.json');
if(process.argv.includes('--write'))fs.writeFileSync(fixturePath,serialized);
else{if(fs.readFileSync(fixturePath,'utf8')!==serialized)throw Error('OAuth1 independent fixtures differ');console.log(`${fixtures.length} Postman OAuth1 vectors match`);}
