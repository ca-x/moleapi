#!/usr/bin/env python3
"""Compile/call original generated C# gRPC artifacts using explicit isolated .NET QA."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

CSHARP = r'''using System.Net;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Fixture;
using Grpc.Core;
using Grpc.Net.Client;
using Microsoft.AspNetCore.Server.Kestrel.Core;
using Microsoft.AspNetCore.Hosting.Server;
using Microsoft.AspNetCore.Hosting.Server.Features;

static class Check {
  public static void Ensure(bool value,string message) { if (!value) throw new InvalidOperationException(message); }
}
sealed class RuntimeService : EchoService.EchoServiceBase {
  public readonly Echo Expected; public readonly Echo Other;
  public readonly List<string> Observations=[];
  public RuntimeService(Echo expected,Echo other) { Expected=expected;Other=other; }
  void Observed(string mode) { lock(Observations) { Observations.Add(mode); } }
  public override Task<Echo> Call(Echo request,ServerCallContext context) {
    Check.Ensure(request.Equals(Expected),"Unary request lost");Observed("unary");return Task.FromResult(request);
  }
  public override async Task Watch(Echo request,IServerStreamWriter<Echo> response,ServerCallContext context) {
    Check.Ensure(request.Equals(Expected),"Server-stream request lost");Observed("server");
    await response.WriteAsync(request);await response.WriteAsync(Other);
  }
  public override async Task<Echo> Collect(IAsyncStreamReader<Echo> input,ServerCallContext context) {
    var requests=new List<Echo>();while(await input.MoveNext(context.CancellationToken)) requests.Add(input.Current);
    Check.Ensure(requests.SequenceEqual(new[]{Expected,Other}),"Client-stream request lost");Observed("client");return requests[1];
  }
  public override async Task Chat(IAsyncStreamReader<Echo> input,IServerStreamWriter<Echo> response,ServerCallContext context) {
    var requests=new List<Echo>();while(await input.MoveNext(context.CancellationToken)) { requests.Add(input.Current);await response.WriteAsync(input.Current); }
    Check.Ensure(requests.SequenceEqual(new[]{Expected,Other}),"Bidi request lost");Observed("bidi");
  }
}
class Program {
  static async Task Main() {
    var expected=new Echo {Text="鼹鼠 \"quotes\" & +",Payload=ByteString.CopyFrom(new byte[]{0,255,128,65}),Shade=Shade.Ready,Note="detail",Observed=new Timestamp {Seconds=1234567,Nanos=123}};
    expected.Labels.Add(new[]{"red","蓝"});var other=expected.Clone();other.Code=0;
    Check.Ensure(expected.DetailCase==Echo.DetailOneofCase.Note && other.DetailCase==Echo.DetailOneofCase.Code,"Oneof zero presence lost");
    var service=new RuntimeService(expected,other);
    var builder=WebApplication.CreateBuilder();builder.Logging.ClearProviders();
    builder.WebHost.ConfigureKestrel(options=>options.Listen(IPAddress.Loopback,0,listen=>listen.Protocols=HttpProtocols.Http2));
    builder.Services.AddGrpc();builder.Services.AddSingleton(service);
    await using var app=builder.Build();app.MapGrpcService<RuntimeService>();await app.StartAsync();
    try {
      var address=app.Services.GetRequiredService<IServer>().Features.Get<IServerAddressesFeature>()!.Addresses.Single();
      using var handler=new SocketsHttpHandler {UseProxy=false};
      using var channel=GrpcChannel.ForAddress(address,new GrpcChannelOptions {HttpHandler=handler});
      var client=new EchoService.EchoServiceClient(channel);
      var unary=await client.CallAsync(expected,deadline:DateTime.UtcNow.AddSeconds(10));Check.Ensure(unary.Equals(expected),"Unary response lost");
      using(var watch=client.Watch(expected,deadline:DateTime.UtcNow.AddSeconds(10))) {
        var received=new List<Echo>();while(await watch.ResponseStream.MoveNext(CancellationToken.None)) received.Add(watch.ResponseStream.Current);
        Check.Ensure(received.SequenceEqual(new[]{expected,other}),"Server-stream response lost");
      }
      using(var collect=client.Collect(deadline:DateTime.UtcNow.AddSeconds(10))) {
        await collect.RequestStream.WriteAsync(expected);await collect.RequestStream.WriteAsync(other);await collect.RequestStream.CompleteAsync();
        Check.Ensure((await collect.ResponseAsync).Equals(other),"Client-stream response lost");
      }
      using(var chat=client.Chat(deadline:DateTime.UtcNow.AddSeconds(10))) {
        await chat.RequestStream.WriteAsync(expected);await chat.RequestStream.WriteAsync(other);await chat.RequestStream.CompleteAsync();
        var received=new List<Echo>();while(await chat.ResponseStream.MoveNext(CancellationToken.None)) received.Add(chat.ResponseStream.Current);
        Check.Ensure(received.SequenceEqual(new[]{expected,other}),"Bidi response lost");
      }
      Check.Ensure(service.Observations.SequenceEqual(new[]{"unary","server","client","bidi"}),"RPC mode/order lost");
      Console.WriteLine("PASS C# gRPC unary/server/client/bidi");
    } finally { using var shutdown=new CancellationTokenSource(TimeSpan.FromSeconds(10));await app.StopAsync(shutdown.Token); }
  }
}
'''

PROJECT='''<Project Sdk="Microsoft.NET.Sdk.Web"><PropertyGroup><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>enable</Nullable><RestorePackagesWithLockFile>true</RestorePackagesWithLockFile></PropertyGroup><ItemGroup>
<PackageReference Include="Google.Protobuf" Version="3.36.1" />
<PackageReference Include="Grpc.Net.Client" Version="2.84.0" />
<PackageReference Include="Grpc.AspNetCore.Server" Version="2.84.0" />
</ItemGroup></Project>
'''


def run(command,cwd,env,timeout=900):
    print('RUN',' '.join(map(str,command)),flush=True)
    subprocess.run(command,cwd=cwd,env=env,check=True,timeout=timeout)


def verify(args):
    root=args.root
    assert root.is_absolute() and root.is_dir()
    assert args.dotnet.is_absolute() and args.dotnet.is_file()
    assert args.packages.is_absolute() and args.cli_home.is_absolute()
    manifest=json.loads((root/'moleapi-generation.json').read_text())
    assert manifest['target']=='protobuf-csharp' and 'grpc-plugin=true' in manifest['engine']
    before={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob('*.cs') if 'runtime-consumer' not in p.relative_to(root).parts}
    consumer=root/'runtime-consumer';consumer.mkdir(exist_ok=True)
    for path in before:
        destination=consumer/'generated'/path;destination.parent.mkdir(parents=True,exist_ok=True);destination.write_bytes((root/path).read_bytes())
    (consumer/'Program.cs').write_text(CSHARP);(consumer/'RuntimeFixture.csproj').write_text(PROJECT)
    (consumer/'NuGet.Config').write_text('<configuration><packageSources><clear/><add key="nuget.org" value="https://api.nuget.org/v3/index.json"/></packageSources></configuration>\n')
    env=dict(os.environ,DOTNET_CLI_HOME=str(args.cli_home),NUGET_PACKAGES=str(args.packages),DOTNET_SKIP_FIRST_TIME_EXPERIENCE='1',DOTNET_CLI_TELEMETRY_OPTOUT='1')
    run([str(args.dotnet),'restore','--configfile','NuGet.Config'],consumer,env)
    run([str(args.dotnet),'build','--configuration','Release','--no-restore'],consumer,env)
    run([str(args.dotnet),str(consumer/'bin/Release/net10.0/RuntimeFixture.dll')],consumer,env,timeout=120)
    dependencies=subprocess.check_output([str(args.dotnet),'list','package','--include-transitive','--format','json'],cwd=consumer,env=env,text=True)
    (consumer/'runtime-dependencies.json').write_text(dependencies)
    assert all(hashlib.sha256((root/path).read_bytes()).hexdigest()==digest for path,digest in before.items()),'Generated C# sources changed'
    evidence={'target':'protobuf-csharp','representation':root.name,'engine':manifest['engine'],'grpc_net':'2.84.0','protobuf':'3.36.1','dotnet':subprocess.check_output([str(args.dotnet),'--info'],env=env,text=True).strip(),'modes':['unary','server','client','bidi'],'descriptor_sha256':hashlib.sha256((root/'descriptor-set.bin').read_bytes()).hexdigest(),'generated_source_sha256':before,'lock_sha256':hashlib.sha256((consumer/'packages.lock.json').read_bytes()).hexdigest(),'result':'passed'}
    (root/'runtime-evidence.json').write_text(json.dumps(evidence,indent=2)+'\n')
    print(json.dumps(evidence,indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root',type=Path)
    parser.add_argument('--dotnet',type=Path,required=True)
    parser.add_argument('--packages',type=Path,required=True)
    parser.add_argument('--cli-home',type=Path,required=True)
    verify(parser.parse_args())
