#!/usr/bin/env python3
"""Compile/call trusted generated SDKs and start skeletons; never invoked by the app."""
import argparse
import hashlib
import http.server
import json
import os
import re
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

REPO = Path(__file__).resolve().parents[2]
TOKEN = "fixture-token"
QUERY = "space & + 鼹鼠"
NOTE = '鼹鼠 "quotes" & +'
PET = {"id": 7, "status": "ready", "note": NOTE}
BINARY = bytes([0,1,2,13,10,255,128,65])


def run(command, cwd, env=None, timeout=300):
    print("RUN", " ".join(map(str, command)), flush=True)
    subprocess.run(command, cwd=cwd, env=env, check=True, timeout=timeout)


class Fixture(http.server.BaseHTTPRequestHandler):
    observations = []
    errors = []

    def log_message(self, *_):
        pass

    def respond(self, status, body):
        payload = json.dumps(body, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def request(self):
        parsed = urllib.parse.urlsplit(self.path)
        body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        self.observations.append((self.command, parsed.path))
        try:
            assert self.headers.get("Authorization") == f"Bearer {TOKEN}"
            if parsed.path == "/fail":
                assert self.command == "GET"
                self.respond(400, {"message": "expected fixture failure"})
                return
            if parsed.path == "/files":
                if self.command == "GET":
                    self.send_response(200)
                    self.send_header("Content-Type", "application/octet-stream")
                    self.send_header("Content-Length", str(len(BINARY)))
                    self.send_header("Content-Disposition", 'attachment; filename="fixture.bin"')
                    self.end_headers()
                    self.wfile.write(BINARY)
                else:
                    assert self.command == "POST"
                    assert self.headers.get_content_type() == "application/octet-stream"
                    assert body == BINARY, body
                    self.respond(200,{"size":len(body)})
                return
            assert parsed.path == "/pets/7", parsed.path
            if self.command == "GET":
                assert urllib.parse.parse_qs(parsed.query) == {"q": [QUERY]}, parsed.query
                assert self.headers.get("X-Test") == "moleapi-fixture"
                self.respond(200, {"id": 7, "status": "ready", "note": None})
            else:
                assert self.command == "POST"
                assert self.headers.get_content_type() == "application/json"
                assert json.loads(body) == PET, body
                self.respond(200, PET)
        except Exception as error:
            self.errors.append(repr(error))
            self.respond(422, {"message": repr(error)})

    do_GET = request
    do_POST = request


GO = r'''package openapi
import (
    "context"
    "encoding/json"
    "os"
    "net/http"
    "testing"
    "time"
)
func TestRuntimeFixture(t *testing.T) {
    config := NewConfiguration()
    config.Servers = ServerConfigurations{{URL:os.Getenv("FIXTURE_URL")}}
    config.HTTPClient = &http.Client{Timeout: 10*time.Second}
    defer config.HTTPClient.CloseIdleConnections()
    api := NewAPIClient(config)
    ctx := context.WithValue(context.Background(), ContextAccessToken, "fixture-token")
    pet, response, err := api.DefaultAPI.GetPet(ctx,7).Q("space & + 鼹鼠").XTest("moleapi-fixture").Execute()
    if err!=nil || response.StatusCode!=200 || pet.GetId()!=7 || pet.GetStatus()!="ready" {t.Fatalf("GET: %v %v %v",pet,response,err)}
    encoded,err:=json.Marshal(pet); if err!=nil {t.Fatal(err)}
    var decoded map[string]any; if err=json.Unmarshal(encoded,&decoded); err!=nil {t.Fatal(err)}
    if note,found:=decoded["note"]; !found || note!=nil {t.Fatalf("nullable note not preserved: %s",encoded)}
    input:=NewPet(7,"ready"); input.SetNote("鼹鼠 \"quotes\" & +")
    pet,response,err=api.DefaultAPI.ReplacePet(ctx,7).Pet(*input).Execute()
    if err!=nil || response.StatusCode!=200 || pet.GetNote()!="鼹鼠 \"quotes\" & +" {t.Fatalf("POST: %v %v %v",pet,response,err)}
    response,err=api.DefaultAPI.FailGet(ctx).Execute()
    if err==nil || response==nil || response.StatusCode!=400 {t.Fatalf("error response lost: %v %v",response,err)}
    __GO_BINARY__
}
'''
PYTHON = r'''import os
import re
from openapi_client import ApiClient, Configuration, Pet
from openapi_client.api.default_api import DefaultApi
from openapi_client.exceptions import ApiException
with ApiClient(Configuration(host=os.environ["FIXTURE_URL"], access_token="fixture-token")) as client:
    api=DefaultApi(client)
    pet=api.get_pet(id=7, q="space & + 鼹鼠", x_test="moleapi-fixture")
    assert pet.id==7 and pet.status=="ready" and pet.note is None, pet
    pet=api.replace_pet(id=7, pet=Pet(id=7,status="ready",note='鼹鼠 "quotes" & +'))
    assert pet.note=='鼹鼠 "quotes" & +', pet
    try:
        api.fail_get()
    except ApiException as error:
        assert error.status==400 and "expected fixture failure" in error.body, error
    else:
        raise AssertionError("HTTP error was not exposed")
    __PYTHON_BINARY__
print("PASS Python client")
'''
JAVASCRIPT = r'''const assert=require('node:assert/strict');
const sdk=require('./dist/index.js');
const client=new sdk.ApiClient(); client.basePath=process.env.FIXTURE_URL;
client.authentications.bearerAuth.accessToken='fixture-token';
const api=new sdk.DefaultApi(client);
function call(name,...args) {return new Promise((resolve,reject)=>api[name](...args,(error,data,response)=>error?reject(error):resolve(data)));}
(async()=>{
  let pet=await call('getPet',__GET_PET_ARGS__);
  assert.equal(pet.id,7); assert.equal(pet.status,'ready'); assert.equal(pet.note,null);
  pet=await call('replacePet',7,sdk.Pet.constructFromObject({id:7,status:'ready',note:'鼹鼠 "quotes" & +'}));
  assert.equal(pet.note,'鼹鼠 "quotes" & +');
  await assert.rejects(call('failGet'),error=>error.status===400 && error.response.body.message==='expected fixture failure');
  __JS_BINARY__
  console.log('PASS JavaScript client');
})().catch(error=>{console.error(error);process.exit(1)});
'''
TYPESCRIPT = r'''const assert=require('node:assert/strict');
const sdk=require('./dist/index.js');
const api=new sdk.DefaultApi(new sdk.Configuration({basePath:process.env.FIXTURE_URL,accessToken:'fixture-token'}));
(async()=>{
  let pet=await api.getPet({id:7,q:'space & + 鼹鼠',xTest:'moleapi-fixture'});
  assert.equal(pet.id,7); assert.equal(pet.status,'ready'); assert.equal(pet.note,null);
  pet=await api.replacePet({id:7,pet:{id:7,status:'ready',note:'鼹鼠 "quotes" & +'}});
  assert.equal(pet.note,'鼹鼠 "quotes" & +');
  await assert.rejects(api.failGet(),error=>error instanceof sdk.ResponseError && error.response.status===400);
  __TS_BINARY__
  console.log('PASS TypeScript client');
})().catch(error=>{console.error(error);process.exit(1)});
'''


CSHARP = r'''using System;
using System.Net;
using System.Threading;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.DependencyInjection;
using Org.OpenAPITools.Api;
using Org.OpenAPITools.Client;
using Org.OpenAPITools.Model;
using Org.OpenAPITools.Extensions;

using var cancel = new CancellationTokenSource(TimeSpan.FromSeconds(20));
var builder = Host.CreateApplicationBuilder();
builder.__CONFIGURE_API__(options => {
    options.AddTokens(new BearerToken("fixture-token"));
    options.__ADD_HTTP_CLIENTS__(client => {
        client.BaseAddress = new Uri(Environment.GetEnvironmentVariable("FIXTURE_URL")!);
        client.Timeout = TimeSpan.FromSeconds(10);
    });
});
using var host = builder.Build();
var api = host.Services.GetRequiredService<IDefaultApi>();
var response = await api.GetPetAsync(id: 7, q: "space & + 鼹鼠", xTest: "moleapi-fixture", cancellationToken: cancel.Token);
var pet = response.Ok()!;
if (response.StatusCode != HttpStatusCode.OK || pet.Id != 7 || pet.Status != Pet.StatusEnum.Ready || pet.Note != null)
    throw new Exception("GET model or nullable value lost");
var input = new Pet(id: 7, status: Pet.StatusEnum.Ready) { Note = "鼹鼠 \"quotes\" & +" };
var post = await api.ReplacePetAsync(id: 7, pet: input, cancellationToken: cancel.Token);
if (post.StatusCode != HttpStatusCode.OK || post.Ok()!.Note != "鼹鼠 \"quotes\" & +")
    throw new Exception("POST response/body lost");
var failure = await api.FailGetAsync(cancellationToken: cancel.Token);
if (failure.StatusCode != HttpStatusCode.BadRequest || failure.BadRequest()!.Message != "expected fixture failure")
    throw new Exception("HTTP400 model/status lost");
__CS_BINARY__
Console.WriteLine("PASS C# generichost client");
'''


GO_BINARY = r'''data:=[]byte{0,1,2,13,10,255,128,65}
    downloaded,response,err:=api.DefaultAPI.DownloadFile(ctx).Execute()
    if err!=nil || response.StatusCode!=200 {t.Fatalf("download: %v %v",response,err)}
    defer downloaded.Close(); defer os.Remove(downloaded.Name())
    raw,err:=io.ReadAll(downloaded)
    if err!=nil || !bytes.Equal(raw,data) {t.Fatalf("download bytes: %x %v",raw,err)}
    upload,err:=os.CreateTemp(t.TempDir(),"upload*.bin"); if err!=nil {t.Fatal(err)}
    defer upload.Close()
    if _,err=upload.Write(data); err!=nil {t.Fatal(err)}
    if _,err=upload.Seek(0,0); err!=nil {t.Fatal(err)}
    receipt,response,err:=api.DefaultAPI.UploadFile(ctx).Body(upload).Execute()
    if err!=nil || response.StatusCode!=200 || receipt.GetSize()!=int32(len(data)) {t.Fatalf("upload: %v %v %v",receipt,response,err)}
'''
PYTHON_BINARY = r'''data=bytes([0,1,2,13,10,255,128,65])
    downloaded=api.download_file()
    if isinstance(downloaded,str):
        from pathlib import Path
        path=Path(downloaded)
        try: raw=path.read_bytes()
        finally: path.unlink()
    else:
        raw=bytes(downloaded)
    assert raw==data, raw
    receipt=api.upload_file(body=data)
    assert receipt.size==len(data), receipt
'''
JS_BINARY = r'''const data=Buffer.from([0,1,2,13,10,255,128,65]);
  const downloaded=await call('downloadFile');
  assert.ok(Buffer.isBuffer(downloaded), 'download is not raw binary');
  assert.deepEqual(downloaded,data);
  const receipt=await call('uploadFile',data);
  assert.equal(receipt.size,data.length);
'''
TS_BINARY = r'''const data=Buffer.from([0,1,2,13,10,255,128,65]);
  const downloaded=await api.downloadFile();
  assert.deepEqual(Buffer.from(await downloaded.arrayBuffer()),data);
  const receipt=await api.uploadFile({body:new Blob([data],{type:'application/octet-stream'})});
  assert.equal(receipt.size,data.length);
'''
CS_BINARY = r'''byte[] data = {0,1,2,13,10,255,128,65};
var downloaded = await api.DownloadFileAsync(cancellationToken: cancel.Token);
using var stream = downloaded.Ok()!;
using var buffer = new System.IO.MemoryStream();
await stream.CopyToAsync(buffer,cancel.Token);
if (!System.Linq.Enumerable.SequenceEqual(buffer.ToArray(),data)) throw new Exception("Binary download changed bytes");
using var upload = new System.IO.MemoryStream(data);
var receipt = await api.UploadFileAsync(body: new FileParameter(upload), cancellationToken: cancel.Token);
if (receipt.StatusCode != HttpStatusCode.OK || receipt.Ok()!.Size != data.Length) throw new Exception("Binary upload receipt failed");
'''


ASPNET_HOST = r'''using System;
using System.Security.Claims;
using System.Text.Encodings.Web;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;

await Host.CreateDefaultBuilder(args).ConfigureWebHostDefaults(web => web.UseStartup<FixtureStartup>()).Build().RunAsync();
public class FixtureStartup {
    readonly Org.OpenAPITools.Startup generated;
    public FixtureStartup(IConfiguration config) { generated = new(config); }
    public void ConfigureServices(IServiceCollection services) {
        generated.ConfigureServices(services);
        // Test-only consumer configuration. No generated source is changed.
        services.AddAuthentication("Fixture").AddScheme<AuthenticationSchemeOptions, FixtureAuth>("Fixture", _ => {});
    }
    public void Configure(IApplicationBuilder app, IWebHostEnvironment env) { generated.Configure(app, env); }
}
public class FixtureAuth : AuthenticationHandler<AuthenticationSchemeOptions> {
    public FixtureAuth(IOptionsMonitor<AuthenticationSchemeOptions> options, ILoggerFactory logger, UrlEncoder encoder) : base(options,logger,encoder) {}
    protected override Task<AuthenticateResult> HandleAuthenticateAsync() {
        if (Request.Headers.Authorization != "Bearer fixture-token")
            return Task.FromResult(AuthenticateResult.Fail("Missing fixture token"));
        var identity = new ClaimsIdentity(new[]{new Claim(ClaimTypes.Name,"fixture")},Scheme.Name);
        return Task.FromResult(AuthenticateResult.Success(new AuthenticationTicket(new ClaimsPrincipal(identity),Scheme.Name)));
    }
}
'''


def dotnet_project(directory):
    project = directory / "src/Org.OpenAPITools/Org.OpenAPITools.csproj"
    assert project.is_file(), "Generated default project missing"
    return project


def csharp(directory, url, args):
    assert args.dotnet, "Configure --dotnet for .NET validation"
    project = dotnet_project(directory)
    run([args.dotnet, "build", str(project), "-c", "Release"], directory)
    extensions = "\n".join(path.read_text() for path in project.parent.glob("Extensions/*.cs"))
    match = re.search(r"Configure(\w+)\(", extensions)
    assert match, "Generated hosting extension missing"
    api_name = match.group(1)
    driver = CSHARP.replace("__CONFIGURE_API__", "Configure" + api_name).replace("__ADD_HTTP_CLIENTS__", "Add" + api_name + "HttpClients")
    driver = driver.replace("__CS_BINARY__", CS_BINARY if args.profile=="binary" else "")
    consumer = directory / "runtime-consumer"
    consumer.mkdir(exist_ok=True)
    (consumer / "Program.cs").write_text(driver)
    (consumer / "RuntimeFixture.csproj").write_text(
        '<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><Nullable>enable</Nullable></PropertyGroup><ItemGroup><ProjectReference Include="'
        + str(project) + '" /></ItemGroup></Project>')
    run([args.dotnet, "build", "-c", "Release"], consumer)
    run([args.dotnet, "run", "--no-build", "-c", "Release"], consumer, dict(os.environ, FIXTURE_URL=url))


def stop(process):
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def aspnetcore(directory, args):
    assert args.dotnet, "Configure --dotnet for .NET validation"
    project = dotnet_project(directory)
    run([args.dotnet, "build", str(project), "-c", "Release"], directory)
    assemblies = list((project.parent / "bin/Release").glob("*/Org.OpenAPITools.dll"))
    assert len(assemblies) == 1, assemblies
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    url = f"http://127.0.0.1:{port}"
    with (directory / "runtime-server.log").open("w") as log:
        process = subprocess.Popen([args.dotnet, str(assemblies[0]), "--urls", url], cwd=project.parent, stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 30
            while True:
                if process.poll() is not None:
                    raise AssertionError((directory / "runtime-server.log").read_text())
                try:
                    with urllib.request.urlopen(url + "/openapi/1/openapi.json", timeout=1) as reply:
                        assert reply.status == 200
                        definition = json.load(reply)
                        assert "/pets/{id}" in definition["paths"] and "/fail" in definition["paths"]
                    break
                except (OSError, urllib.error.URLError):
                    if time.monotonic() >= deadline:
                        raise AssertionError((directory / "runtime-server.log").read_text())
                    time.sleep(0.1)
            request = urllib.request.Request(url + "/pets/7?" + urllib.parse.urlencode({"q": QUERY}), headers={"Authorization": f"Bearer {TOKEN}", "X-Test": "moleapi-fixture"})
            try:
                with urllib.request.urlopen(request, timeout=5) as reply:
                    assert reply.status == 200
                    body = json.load(reply)
                    assert "id" in body and "status" in body
            except urllib.error.HTTPError as error:
                assert error.code == 500, error.code
                log.flush()
                assert "No authenticationScheme was specified" in (directory / "runtime-server.log").read_text()
        finally:
            stop(process)
    consumer = directory / "runtime-host"
    consumer.mkdir(exist_ok=True)
    (consumer / "Program.cs").write_text(ASPNET_HOST)
    (consumer / "RuntimeHost.csproj").write_text('<Project Sdk="Microsoft.NET.Sdk.Web"><PropertyGroup><TargetFramework>net10.0</TargetFramework><Nullable>enable</Nullable></PropertyGroup><ItemGroup><ProjectReference Include="' + str(project) + '" /></ItemGroup></Project>')
    run([args.dotnet, "build", "-c", "Release"], consumer)
    with (directory / "runtime-consumer-host.log").open("w") as log:
        process = subprocess.Popen([args.dotnet, str(consumer / "bin/Release/net10.0/RuntimeHost.dll"), "--urls", url], cwd=project.parent, stdout=log, stderr=log)
        try:
            deadline = time.monotonic()+30
            while True:
                if process.poll() is not None:
                    raise AssertionError((directory / "runtime-consumer-host.log").read_text())
                try:
                    urllib.request.urlopen(url + "/openapi/1/openapi.json", timeout=1).close()
                    break
                except (OSError, urllib.error.URLError):
                    if time.monotonic() >= deadline:
                        raise AssertionError("Consumer host did not start")
                    time.sleep(0.1)
            route = url + "/pets/7?" + urllib.parse.urlencode({"q": QUERY})
            request = urllib.request.Request(route, headers={"Authorization":f"Bearer {TOKEN}", "X-Test":"moleapi-fixture"})
            with urllib.request.urlopen(request, timeout=5) as reply:
                assert reply.status == 200
                body = json.load(reply)
                assert "id" in body and "status" in body, body
            try:
                urllib.request.urlopen(urllib.request.Request(route, headers={"X-Test":"moleapi-fixture"}), timeout=5)
            except urllib.error.HTTPError as error:
                assert error.code == 401, error.code
            else:
                raise AssertionError("Protected generated route accepted anonymous request")
        finally:
            stop(process)
    print("PASS ASP.NET Core default build/start/routes plus authenticated200/anonymous401 through consumer scheme", flush=True)


def python_environment(directory):
    run([sys.executable, "-m", "venv", str(directory / ".venv")], directory)
    python = directory / ".venv" / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
    run([str(python), "-m", "pip", "install", "."], directory)
    freeze = subprocess.check_output([str(python), "-m", "pip", "freeze"], cwd=directory, text=True)
    (directory / "runtime-consumer-dependencies.txt").write_text(freeze)
    return python


def client(target, directory, url, args):
    env = dict(os.environ, FIXTURE_URL=url)
    if target == "go":
        driver = GO.replace("__GO_BINARY__", GO_BINARY if args.profile=="binary" else "")
        if args.profile=="binary":
            driver=driver.replace('"context"', '"context"\n    "bytes"\n    "io"',1)
        (directory / "runtime_fixture_test.go").write_text(driver)
        run([args.go, "mod", "tidy"], directory)
        run([args.go, "test", "-run", "^TestRuntimeFixture$", "-count=1", "-race", "-v"], directory, env)
    elif target == "csharp":
        csharp(directory, url, args)
    elif target == "python":
        python = python_environment(directory)
        (directory / "runtime_fixture.py").write_text(PYTHON.replace("__PYTHON_BINARY__", PYTHON_BINARY if args.profile=="binary" else ""))
        run([str(python), "runtime_fixture.py"], directory, env)
    else:
        run([args.npm, "install", "--no-audit", "--no-fund"], directory)
        run([args.npm, "run", "build"], directory)
        driver = TYPESCRIPT
        if target == "javascript":
            source = (directory / "src/api/DefaultApi.js").read_text()
            signature = re.search(r"getPet\(([^)]+)\)", source)
            assert signature, "Generated getPet signature missing"
            names = [name.strip() for name in signature.group(1).split(",")]
            assert names[-1] == "callback" and set(names[:-1]) == {"id","q","xTest"}, names
            values = {"id": "7", "q": json.dumps(QUERY), "xTest": json.dumps("moleapi-fixture")}
            driver = JAVASCRIPT.replace("__GET_PET_ARGS__", ",".join(values[name] for name in names[:-1]))
        driver=driver.replace("__JS_BINARY__", JS_BINARY if args.profile=="binary" else "").replace("__TS_BINARY__", TS_BINARY if args.profile=="binary" else "")
        (directory / "runtime_fixture.cjs").write_text(driver)
        run([args.node, "runtime_fixture.cjs"], directory, env)


def fastapi(directory):
    python = python_environment(directory)
    run([str(python), "-m", "pip", "install", "uvicorn"], directory)
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    url = f"http://127.0.0.1:{port}"
    with (directory / "runtime-server.log").open("w") as log:
        process = subprocess.Popen([str(python), "-m", "uvicorn", "openapi_server.main:app", "--host", "127.0.0.1", "--port", str(port)], cwd=directory, stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 30
            while True:
                if process.poll() is not None:
                    raise AssertionError((directory / "runtime-server.log").read_text())
                try:
                    with urllib.request.urlopen(url + "/openapi.json", timeout=1) as reply:
                        assert reply.status == 200
                        definition = json.load(reply)
                        assert "/pets/{id}" in definition["paths"] and "/fail" in definition["paths"]
                    break
                except (OSError, urllib.error.URLError):
                    if time.monotonic() >= deadline:
                        raise AssertionError("Generated FastAPI server did not become ready")
                    time.sleep(0.1)
            request = urllib.request.Request(url + "/pets/7?" + urllib.parse.urlencode({"q": QUERY}), headers={"Authorization": f"Bearer {TOKEN}", "X-Test": "moleapi-fixture"})
            try:
                urllib.request.urlopen(request, timeout=5)
            except urllib.error.HTTPError as error:
                assert error.code == 500 and json.load(error) == {"detail": "Not implemented"}
            else:
                raise AssertionError("Expected unimplemented business-handler response")
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
    print("PASS FastAPI skeleton startup/routes and explicit missing-handler response", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--worker", type=Path, required=True)
    parser.add_argument("--java", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--targets", default="javascript,python,go,typescript-fetch,python-fastapi")
    parser.add_argument("--versions", default="3.0.3,3.1.0")
    parser.add_argument("--profile", choices=["core","binary"], default="core")
    parser.add_argument("--node", default=shutil.which("node"))
    parser.add_argument("--npm", default=shutil.which("npm"))
    parser.add_argument("--go", default=shutil.which("go"))
    parser.add_argument("--dotnet", default=shutil.which("dotnet"))
    parser.add_argument("--reuse-generated", action="store_true", help="Use only previously generated isolated fixtures")
    args = parser.parse_args()
    assert args.worker.is_absolute() and args.worker.is_file()
    assert args.java.is_absolute() and args.java.is_file()
    assert args.output.is_absolute()
    targets = args.targets.split(",")
    versions = args.versions.split(",")
    assert all(t in {"javascript", "python", "go", "typescript-fetch", "python-fastapi", "csharp", "aspnetcore"} for t in targets)
    assert all(v in {"3.0.3", "3.1.0"} for v in versions)
    if args.profile=="binary":
        assert not {"python-fastapi","aspnetcore"}.intersection(targets), "Binary profile currently validates clients"
    if not args.reuse_generated:
        run(["cargo", "run", "-p", "moleapi-generation", "--example", "fixture_projects", "--", str(args.output), str(args.worker), str(args.java), args.targets, args.versions, args.profile], REPO, timeout=300)
    evidence_path = args.output / "runtime-evidence.json"
    evidence = json.loads(evidence_path.read_text()) if evidence_path.exists() else []
    evidence = [record for record in evidence if (record["target"], record["dialect"], record.get("profile","core")) not in {(t,v,args.profile) for t in targets for v in versions}]
    evidence_path.write_text(json.dumps(evidence, indent=2) + "\n")
    tools = {"python": sys.version, "java": subprocess.check_output([str(args.java), "-version"], stderr=subprocess.STDOUT, text=True).strip()}
    for name, command in [("node", args.node), ("npm", args.npm), ("go", args.go)]:
        if command:
            tools[name] = subprocess.check_output([command, "version" if name=="go" else "--version"], text=True).strip()
    if args.dotnet:
        tools["dotnet"] = subprocess.check_output([args.dotnet, "--info"], text=True).strip()
    (args.output / "runtime-tools.json").write_text(json.dumps(tools, indent=2) + "\n")
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        for version in versions:
            for target in targets:
                directory = args.output / version / target
                print(f"VERIFY {target}/{version}", flush=True)
                Fixture.observations.clear()
                Fixture.errors.clear()
                if target == "python-fastapi":
                    fastapi(directory)
                elif target == "aspnetcore":
                    aspnetcore(directory, args)
                else:
                    client(target, directory, f"http://127.0.0.1:{server.server_port}", args)
                    assert not Fixture.errors, Fixture.errors
                    expected=[("GET", "/pets/7"), ("POST", "/pets/7"), ("GET", "/fail")]
                    if args.profile=="binary": expected += [("GET","/files"),("POST","/files")]
                    assert Fixture.observations == expected, Fixture.observations
                manifest = json.loads((directory / "moleapi-generation.json").read_text())
                source_digest = hashlib.sha256((directory / "fixture-source.json").read_bytes()).hexdigest()
                evidence.append({"target": target, "dialect": version, "profile": args.profile, "engine": manifest["engine"], "fixture_sha256": source_digest, "result": "passed"})
                (args.output / "runtime-evidence.json").write_text(json.dumps(evidence, indent=2) + "\n")
                print(f"PASS {target}/{version}", flush=True)
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
    print(json.dumps(evidence, indent=2))


if __name__ == "__main__":
    main()
