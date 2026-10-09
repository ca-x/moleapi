"""Run only trusted generated multipart fixtures, including binary file bytes.
Requires Node/Python requests/Go/cURL/.NET10 and cached RestSharp114 packages.
Pass --node-modules and --dotnet as explicit absolute development tool paths.
"""
import argparse
import base64
from email.parser import BytesParser
from email.policy import default
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import sys
import tempfile
import threading
import urllib.parse
ROOT=Path(__file__).resolve().parents[2]
parser=argparse.ArgumentParser()
parser.add_argument('--node-modules',required=True,type=Path)
parser.add_argument('--dotnet',required=True,type=Path)
parser.add_argument('--clients',default='node,native,request,unirest,go,curl,python,httpclient,restsharp')
args=parser.parse_args()
clients=set(args.clients.split(','))
for path in [args.node_modules,args.dotnet]:
    if not path.is_absolute() or not path.exists():parser.error('select existing absolute tool paths')
records=queue.Queue()
def run(command,cwd,env=None):
    result=subprocess.run(command,cwd=cwd,env=env,text=True,capture_output=True,timeout=180)
    if result.returncode:raise RuntimeError(result.stdout+result.stderr)
def check(name):
    record=records.get(timeout=10)
    assert record['method']=='POST',record
    headers=record['headers']
    assert headers['authorization']=='Bearer fixture-token' and headers['x-test']=='moleapi-fixture',headers
    assert urllib.parse.parse_qs(urllib.parse.urlsplit(record['path']).query)=={'existing':['one'],'q':['space & +']},record['path']
    message=BytesParser(policy=default).parsebytes(('Content-Type: '+headers['content-type']+'\r\nMIME-Version: 1.0\r\n\r\n').encode()+base64.b64decode(record['body']))
    assert message.is_multipart(),message
    parts=list(message.iter_parts())
    labels=[part.get_payload(decode=True).decode('utf8') for part in parts if part.get_param('name',header='content-disposition')=='label']
    assert labels==["鼹鼠 'quotes' & +",'second'],(name,labels)
    uploads=[part for part in parts if part.get_param('name',header='content-disposition')=='upload']
    assert len(uploads)==1,(name,[(part.get_filename(),part.get_param('name',header='content-disposition')) for part in parts])
    assert uploads[0].get_payload(decode=True)==bytes([0,255,128,10,88]),(name,uploads[0].get_payload(decode=True))
    assert uploads[0].get_filename()=='upload.bin' and uploads[0].get_content_type()=='application/octet-stream'
    assert len(parts)==3,(name,parts)
    print(f'PASS {name}: real POST/auth/query/repeated Unicode fields/binary filename/MIME')
with tempfile.TemporaryDirectory(prefix='moleapi-multipart-runtime-') as temporary:
    output=Path(temporary)
    (output/'server.cjs').write_text("const http=require('http');const server=http.createServer((request,response)=>{const chunks=[];request.on('data',chunk=>chunks.push(chunk));request.on('end',()=>{console.log(JSON.stringify({method:request.method,path:request.url,headers:request.headers,body:Buffer.concat(chunks).toString('base64')}));response.writeHead(200,{'Content-Type':'application/json'});response.end('{\"ok\":true}');});});server.listen(0,'127.0.0.1',()=>console.log(JSON.stringify({port:server.address().port})));\n")
    server=subprocess.Popen(['node',str(output/'server.cjs')],stdout=subprocess.PIPE,text=True)
    try:
        port=json.loads(server.stdout.readline())['port']
        def receive():
            for line in server.stdout:records.put(json.loads(line))
        threading.Thread(target=receive,daemon=True).start()
        run(['cargo','run','-p','moleapi-generation','--example','fixture_snippets','--locked','--',str(output),f'http://127.0.0.1:{port}','multipart'],ROOT)
        (output/'upload.bin').write_bytes(bytes([0,255,128,10,88]))
        for client,name in [('node','node.mjs'),('native','native.cjs'),('request','request.cjs'),('unirest','unirest.cjs')]:
            if client in clients:run(['node',name],output,{**os.environ,'NODE_PATH':str(args.node_modules)});check(name)
        if 'go' in clients:run(['go','run','main.go'],output);check('Go')
        if 'curl' in clients:run(['sh','curl.sh'],output);check('cURL')
        if 'python' in clients:
            (output/'python.py').write_text('import requests\n'+(output/'python.py').read_text())
            run([sys.executable,'python.py'],output);check('Python Requests')
        for name in ['httpclient','restsharp']:
            if name not in clients:continue
            project=output/name;project.mkdir();shutil.copy(output/'upload.bin',project/'upload.bin')
            dependency='<ItemGroup><PackageReference Include="RestSharp" Version="114.0.0" /></ItemGroup>' if name=='restsharp' else ''
            (project/'fixture.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><LangVersion>11</LangVersion></PropertyGroup>'+dependency+'</Project>')
            imports='using System.Net.Http.Headers;\n'+('using RestSharp;\n' if name=='restsharp' else '')
            (project/'Program.cs').write_text(imports+(output/(name+'.cs')).read_text())
            run([str(args.dotnet),'run','--project','fixture.csproj','--verbosity','quiet'],project,{**os.environ,'DOTNET_NOLOGO':'1','DOTNET_SKIP_FIRST_TIME_EXPERIENCE':'1','DOTNET_CLI_TELEMETRY_OPTOUT':'1'});check('C#/'+name)
        assert records.empty()
    finally:
        server.terminate();server.wait(timeout=10)
