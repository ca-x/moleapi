// Independent fixture generator: node scripts/verify_aws_s3_presign.cjs /path/to/aws4
// aws4 is the signing library used by Postman Runtime; it is not bundled in the UI.
const assert=require('node:assert/strict');
const aws4=require(process.argv[2]||'aws4');
const request={host:'examplebucket.s3.amazonaws.com',path:'/a//b?X-Amz-Date=20150830T123600Z&X-Amz-Expires=3600',method:'GET',service:'s3',region:'us-east-1',signQuery:true};
aws4.sign(request,{accessKeyId:'AKIDEXAMPLE',secretAccessKey:'wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY'});
const signature=new URL('https://'+request.host+request.path).searchParams.get('X-Amz-Signature');
assert.equal(signature,'8e214675647ba615e4d898c3ccff63cfa2c4796d3eb35f52ca53f1cc0cee72d3');
console.log(JSON.stringify({signature,s3_unsigned_payload_verified:true}));
