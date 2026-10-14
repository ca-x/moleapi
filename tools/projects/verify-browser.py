#!/usr/bin/env python3
"""Run trusted generated SDKs in an isolated agent-browser Chromium session."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess
import threading
import urllib.parse

REPO=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location("project_runtime_fixture",Path(__file__).with_name("verify-runtime.py"))
runtime=importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)

HTML=b'<!doctype html><html lang="en"><meta charset="utf-8"><title>Generated browser SDK fixture</title><h1>Generated SDK fixture</h1><pre id="result">Running</pre><script src="/sdk.js"></script></html>'


class BrowserFixture(runtime.Fixture):
    bundle=None

    def request(self):
        path=urllib.parse.urlsplit(self.path).path
        if self.command=="GET" and path in {"/harness","/sdk.js","/favicon.ico"}:
            if path=="/favicon.ico":
                self.send_response(204);self.end_headers();return
            data=HTML if path=="/harness" else self.bundle.read_bytes()
            self.send_response(200)
            self.send_header("Content-Type","text/html; charset=utf-8" if path=="/harness" else "text/javascript; charset=utf-8")
            self.send_header("Content-Length",str(len(data)))
            self.end_headers();self.wfile.write(data);return
        super().request()

    do_GET=request
    do_POST=request


JS_CORE=r'''const client=new sdk.ApiClient(); client.basePath=location.origin;
client.authentications.bearerAuth.accessToken='fixture-token';
const api=new sdk.DefaultApi(client);
function call(name,...args) {return new Promise((resolve,reject)=>api[name](...args,(error,data)=>error?reject(error):resolve(data)));}
let pet=await call('getPet',__GET_ARGS__);
assert(pet.id===7 && pet.status==='ready' && pet.note===null,'GET model/nullable data');
pet=await call('replacePet',7,sdk.Pet.constructFromObject({id:7,status:'ready',note:'鼹鼠 "quotes" & +'}));
assert(pet.note==='鼹鼠 "quotes" & +','POST Unicode JSON');
let rejected=false;
try {await call('failGet');} catch(error) {rejected=error.status===400 && error.response.body.message==='expected fixture failure';}
assert(rejected,'HTTP400 not exposed');
'''
TS_CORE=r'''const api=new sdk.DefaultApi(new sdk.Configuration({basePath:location.origin,accessToken:'fixture-token'}));
let pet=await api.getPet({id:7,q:'space & + 鼹鼠',xTest:'moleapi-fixture'});
assert(pet.id===7 && pet.status==='ready' && pet.note===null,'GET model/nullable data');
pet=await api.replacePet({id:7,pet:{id:7,status:'ready',note:'鼹鼠 "quotes" & +'}});
assert(pet.note==='鼹鼠 "quotes" & +','POST Unicode JSON');
let rejected=false;
try {await api.failGet();} catch(error) {rejected=error instanceof sdk.ResponseError && error.response.status===400;}
assert(rejected,'HTTP400 not exposed');
'''
JS_BINARY=r'''const bytes=new Uint8Array([0,1,2,13,10,255,128,65]);
const downloaded=await call('downloadFile');
assert(downloaded instanceof Blob,'download is not browser Blob');
equalBytes(new Uint8Array(await downloaded.arrayBuffer()),bytes);
const receipt=await call('uploadFile',new Blob([bytes],{type:'application/octet-stream'}));
assert(receipt.size===bytes.length,'binary receipt');
'''
TS_BINARY=r'''const bytes=new Uint8Array([0,1,2,13,10,255,128,65]);
const downloaded=await api.downloadFile();
assert(downloaded instanceof Blob,'download is not browser Blob');
equalBytes(new Uint8Array(await downloaded.arrayBuffer()),bytes);
const receipt=await api.uploadFile({body:new Blob([bytes],{type:'application/octet-stream'})});
assert(receipt.size===bytes.length,'binary receipt');
'''
JS_MULTIPART=r'''const bytes=new Uint8Array([0,1,2,13,10,255,128,65]);
const file=new File([bytes],'payload.bin',{type:'application/octet-stream'});
const receipt=await call('uploadMultipart',__MULTIPART_ARGS__);
assert(receipt.size===bytes.length,'multipart receipt');
'''
TS_MULTIPART=r'''const bytes=new Uint8Array([0,1,2,13,10,255,128,65]);
const receipt=await api.uploadMultipart({file:new File([bytes],'payload.bin',{type:'application/octet-stream'}),note:'鼹鼠 "quotes" & +',tags:['red','蓝'],optionalTags:['red','蓝'],packedTags:['red','蓝']});
assert(receipt.size===bytes.length,'multipart receipt');
'''
JS_UNION=r'''const cat=await call('getAnimal','cat');const dog=await call('getAnimal','dog');
assert(cat.getActualInstance() instanceof sdk.Cat && cat.getActualInstance().kind==='cat' && cat.getActualInstance().name==='鼹鼠 "quotes" & +','Cat union');
assert(dog.getActualInstance() instanceof sdk.Dog && dog.getActualInstance().kind==='dog' && dog.getActualInstance().barks===true,'Dog union');
let echo=await call('echoAnimal',cat);assert(echo.getActualInstance() instanceof sdk.Cat && echo.getActualInstance().name===cat.getActualInstance().name,'Cat roundtrip');
echo=await call('echoAnimal',dog);assert(echo.getActualInstance() instanceof sdk.Dog && echo.getActualInstance().barks===true,'Dog roundtrip');
'''
TS_UNION=r'''const cat=await api.getAnimal({kind:'cat'});const dog=await api.getAnimal({kind:'dog'});
assert(cat.kind==='cat' && cat.name==='鼹鼠 "quotes" & +' && cat.barks===undefined,'Cat union');
assert(dog.kind==='dog' && dog.barks===true && dog.name===undefined,'Dog union');
let echo=await api.echoAnimal({animal:cat});assert(echo.kind==='cat' && echo.name===cat.name,'Cat roundtrip');
echo=await api.echoAnimal({animal:dog});assert(echo.kind==='dog' && echo.barks===true,'Dog roundtrip');
'''


def arguments(source,method,values):
    signature=re.search(method+r"\(([^)]+)\)",source)
    assert signature, "Generated JavaScript signature missing: "+method
    names=[part.strip() for part in signature.group(1).split(',')]
    assert names[-1]=='callback' and set(names[:-1])==set(values),names
    return ','.join(values[name] for name in names[:-1])


def entry(target,directory,profile):
    start=JS_CORE if target=='javascript' else TS_CORE
    extra={'core':'','binary':JS_BINARY if target=='javascript' else TS_BINARY,'multipart':JS_MULTIPART if target=='javascript' else TS_MULTIPART,'union':JS_UNION if target=='javascript' else TS_UNION}[profile]
    if target=='javascript':
        source=(directory/'src/api/DefaultApi.js').read_text()
        start=start.replace('__GET_ARGS__',arguments(source,'getPet',{'id':'7','q':json.dumps(runtime.QUERY),'xTest':json.dumps('moleapi-fixture')}))
        if profile=='multipart':
            extra=extra.replace('__MULTIPART_ARGS__',arguments(source,'uploadMultipart',{'file':'file','note':json.dumps(runtime.NOTE),'tags':json.dumps(['red','蓝']),'opts':json.dumps({'optionalTags':['red','蓝'],'packedTags':['red','蓝']})}))
    imported='./src/index.js' if target=='javascript' else './src/index.ts'
    return 'import * as sdk from '+json.dumps(imported)+''';
function assert(value,message) {if (!value) throw new Error(message);}
function equalBytes(actual,expected) {assert(actual.length===expected.length && actual.every((value,index)=>value===expected[index]),'binary bytes changed');}
window.sdkResult={done:false};
window.addEventListener('error',event=>{window.sdkResult={done:true,passed:false,error:String(event.error||event.message)};});
(async()=>{
'''+start+extra+'''
window.sdkResult={done:true,passed:true,userAgent:navigator.userAgent};
})().catch(error=>{window.sdkResult={done:true,passed:false,error:String(error),stack:error.stack};}).finally(()=>{document.getElementById('result').textContent=JSON.stringify(window.sdkResult,null,2);});
'''


def browser_command(args,session,*command,input=None):
    result=subprocess.run([args.browser,'--session',session,*command],cwd=REPO,input=input,text=True,capture_output=True,timeout=50)
    if result.returncode:
        raise AssertionError(result.stdout+'\n'+result.stderr)
    return result.stdout


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--worker',type=Path,required=True)
    parser.add_argument('--java',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--targets',default='javascript,typescript-fetch')
    parser.add_argument('--versions',default='3.0.3,3.1.0')
    parser.add_argument('--profiles',default='core,binary,multipart,union')
    parser.add_argument('--browser',default=shutil.which('agent-browser'))
    parser.add_argument('--esbuild',type=Path,default=REPO/'web/node_modules/.bin/esbuild')
    parser.add_argument('--npm',default=shutil.which('npm'))
    parser.add_argument('--reuse-generated',action='store_true')
    args=parser.parse_args()
    assert args.output.is_absolute() and args.browser and args.esbuild.is_file()
    targets=args.targets.split(',');versions=args.versions.split(',');profiles=args.profiles.split(',')
    assert all(t in {'javascript','typescript-fetch'} for t in targets)
    assert all(v in {'3.0.3','3.1.0'} for v in versions)
    assert all(p in {'core','binary','multipart','union'} for p in profiles)
    session=subprocess.check_output([args.browser,'session','id','--scope','worktree','--prefix','moleapi-sdk-'+args.output.name],cwd=REPO,text=True).strip()
    evidence_path=args.output/'browser-evidence.json'
    evidence=json.loads(evidence_path.read_text()) if evidence_path.exists() else []
    args.output.mkdir(parents=True,exist_ok=True)
    tools={'agentBrowser':subprocess.check_output([args.browser,'--version'],text=True).strip(),'esbuild':subprocess.check_output([str(args.esbuild),'--version'],text=True).strip()}
    (args.output/'browser-tools.json').write_text(json.dumps(tools,indent=2)+'\n')
    requested={(t,v,p) for t in targets for v in versions for p in profiles}
    evidence=[r for r in evidence if (r['target'],r['dialect'],r['profile']) not in requested]
    server=runtime.http.server.ThreadingHTTPServer(('127.0.0.1',0),BrowserFixture)
    thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
    try:
        for profile in profiles:
            output=args.output/profile
            if not args.reuse_generated:
                runtime.run(['cargo','run','-p','moleapi-generation','--example','fixture_projects','--',str(output),str(args.worker),str(args.java),args.targets,args.versions,profile],REPO)
            args.output.mkdir(exist_ok=True)
            evidence_path.write_text(json.dumps(evidence,indent=2)+'\n')
            for version in versions:
                for target in targets:
                    directory=output/version/target
                    print(f'BROWSER VERIFY {target}/{version}/{profile}',flush=True)
                    runtime.run([args.npm,'install','--ignore-scripts','--no-audit','--no-fund'],directory)
                    (directory/'runtime-browser.js').write_text(entry(target,directory,profile))
                    runtime.run([str(args.esbuild),'runtime-browser.js','--bundle','--platform=browser','--format=iife','--outfile=runtime-browser-bundle.js'],directory)
                    BrowserFixture.bundle=directory/'runtime-browser-bundle.js'
                    runtime.Fixture.observations.clear();runtime.Fixture.errors.clear()
                    browser_command(args,session,'open',f'http://127.0.0.1:{server.server_port}/harness')
                    browser_command(args,session,'wait','--fn','window.sdkResult && window.sdkResult.done === true')
                    result=json.loads(browser_command(args,session,'eval','--stdin','--json',input='JSON.stringify(window.sdkResult)'))
                    value=result.get('data',{}).get('result',result.get('result'))
                    result=json.loads(value) if isinstance(value,str) else value
                    assert result and result['passed'],result
                    assert not runtime.Fixture.errors,runtime.Fixture.errors
                    expected=[('GET','/pets/7'),('POST','/pets/7'),('GET','/fail')]
                    if profile=='binary':expected += [('GET','/files'),('POST','/files')]
                    if profile=='multipart':expected += [('POST','/multipart')]
                    if profile=='union':expected += [('GET','/animals/cat'),('GET','/animals/dog'),('POST','/animals'),('POST','/animals')]
                    assert runtime.Fixture.observations==expected,runtime.Fixture.observations
                    manifest=json.loads((directory/'moleapi-generation.json').read_text())
                    evidence.append({'target':target,'dialect':version,'profile':profile,'engine':manifest['engine'],'source_sha256':manifest['source_sha256'],'bundle_sha256':hashlib.sha256(BrowserFixture.bundle.read_bytes()).hexdigest(),'userAgent':result['userAgent'],'result':'passed'})
                    evidence_path.write_text(json.dumps(evidence,indent=2)+'\n')
                    print(f'BROWSER PASS {target}/{version}/{profile}',flush=True)
    finally:
        subprocess.run([args.browser,'--session',session,'close'],cwd=REPO,capture_output=True,text=True,timeout=30)
        server.shutdown();server.server_close();thread.join(timeout=5)
    print(json.dumps(evidence,indent=2))


if __name__=='__main__':main()
