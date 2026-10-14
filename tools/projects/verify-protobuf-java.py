#!/usr/bin/env python3
"""Build/call original generated Java gRPC artifacts with explicit isolated QA tools."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

JAVA = r'''package fixtureqa;
import com.google.protobuf.ByteString;
import com.google.protobuf.Timestamp;
import fixture.Common;
import fixture.Types.Echo;
import fixture.EchoServiceGrpc;
import io.grpc.ManagedChannel;
import io.grpc.Server;
import io.grpc.netty.shaded.io.grpc.netty.NettyChannelBuilder;
import io.grpc.netty.shaded.io.grpc.netty.NettyServerBuilder;
import io.grpc.stub.StreamObserver;
import java.net.InetSocketAddress;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;

public class RuntimeFixture {
  static void check(boolean value,String message) { if (!value) throw new AssertionError(message); }
  static class Received implements StreamObserver<Echo> {
    final List<Echo> values=new ArrayList<>();
    final CompletableFuture<List<Echo>> done=new CompletableFuture<>();
    public void onNext(Echo value) { values.add(value); }
    public void onError(Throwable error) { done.completeExceptionally(error); }
    public void onCompleted() { done.complete(List.copyOf(values)); }
  }
  public static void main(String[] args) throws Exception {
    Echo expected=Echo.newBuilder().setText("鼹鼠 \"quotes\" & +")
      .setPayload(ByteString.copyFrom(new byte[]{0,(byte)255,(byte)128,65}))
      .setShade(Common.Shade.READY).addLabels("red").addLabels("蓝").setNote("detail")
      .setObserved(Timestamp.newBuilder().setSeconds(1234567).setNanos(123)).build();
    Echo other=expected.toBuilder().setCode(0).build();
    check(other.getDetailCase()==Echo.DetailCase.CODE && expected.getDetailCase()==Echo.DetailCase.NOTE,"Oneof zero presence lost");
    List<String> observations=Collections.synchronizedList(new ArrayList<>());
    EchoServiceGrpc.EchoServiceImplBase service=new EchoServiceGrpc.EchoServiceImplBase() {
      @Override public void call(Echo request,StreamObserver<Echo> response) {
        check(request.equals(expected),"Unary request lost"); observations.add("unary");
        response.onNext(request); response.onCompleted();
      }
      @Override public void watch(Echo request,StreamObserver<Echo> response) {
        check(request.equals(expected),"Server-stream request lost"); observations.add("server");
        response.onNext(request); response.onNext(other); response.onCompleted();
      }
      @Override public StreamObserver<Echo> collect(StreamObserver<Echo> response) {
        return new StreamObserver<>() {
          final List<Echo> requests=new ArrayList<>();
          public void onNext(Echo value) { requests.add(value); }
          public void onError(Throwable error) { response.onError(error); }
          public void onCompleted() {
            check(requests.equals(List.of(expected,other)),"Client-stream request lost"); observations.add("client");
            response.onNext(requests.get(1)); response.onCompleted();
          }
        };
      }
      @Override public StreamObserver<Echo> chat(StreamObserver<Echo> response) {
        return new StreamObserver<>() {
          final List<Echo> requests=new ArrayList<>();
          public void onNext(Echo value) { requests.add(value); response.onNext(value); }
          public void onError(Throwable error) { response.onError(error); }
          public void onCompleted() {
            check(requests.equals(List.of(expected,other)),"Bidi request lost"); observations.add("bidi"); response.onCompleted();
          }
        };
      }
    };
    Server server=NettyServerBuilder.forAddress(new InetSocketAddress("127.0.0.1",0)).addService(service).build().start();
    ManagedChannel channel=null;
    try {
      channel=NettyChannelBuilder.forAddress("127.0.0.1",server.getPort()).usePlaintext().build();
      var blocking=EchoServiceGrpc.newBlockingStub(channel).withDeadlineAfter(10,TimeUnit.SECONDS);
      check(blocking.call(expected).equals(expected),"Unary response lost");
      List<Echo> watched=new ArrayList<>(); blocking.watch(expected).forEachRemaining(watched::add);
      check(watched.equals(List.of(expected,other)),"Server-stream response lost");
      var async=EchoServiceGrpc.newStub(channel).withDeadlineAfter(10,TimeUnit.SECONDS);
      Received collected=new Received(); var upload=async.collect(collected);
      upload.onNext(expected); upload.onNext(other); upload.onCompleted();
      check(collected.done.get(10,TimeUnit.SECONDS).equals(List.of(other)),"Client-stream response lost");
      Received chatted=new Received(); var duplex=async.chat(chatted);
      duplex.onNext(expected); duplex.onNext(other); duplex.onCompleted();
      check(chatted.done.get(10,TimeUnit.SECONDS).equals(List.of(expected,other)),"Bidi response lost");
      check(observations.equals(List.of("unary","server","client","bidi")),"RPC mode/order lost: "+observations);
      System.out.println("PASS Java gRPC unary/server/client/bidi");
    } finally {
      if (channel!=null) { channel.shutdownNow(); check(channel.awaitTermination(10,TimeUnit.SECONDS),"Channel cleanup failed"); }
      server.shutdownNow(); check(server.awaitTermination(10,TimeUnit.SECONDS),"Server cleanup failed");
    }
  }
}
'''

POM = '''<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion>
<groupId>fixtureqa</groupId><artifactId>RuntimeFixture</artifactId><version>1</version>
<properties><maven.compiler.release>21</maven.compiler.release><project.build.sourceEncoding>UTF-8</project.build.sourceEncoding></properties>
<dependencies>
<dependency><groupId>io.grpc</groupId><artifactId>grpc-netty-shaded</artifactId><version>1.84.1</version></dependency>
<dependency><groupId>io.grpc</groupId><artifactId>grpc-protobuf</artifactId><version>1.84.1</version></dependency>
<dependency><groupId>io.grpc</groupId><artifactId>grpc-stub</artifactId><version>1.84.1</version></dependency>
<dependency><groupId>com.google.protobuf</groupId><artifactId>protobuf-java</artifactId><version>4.36.1</version></dependency>
<dependency><groupId>javax.annotation</groupId><artifactId>javax.annotation-api</artifactId><version>1.3.2</version></dependency>
</dependencies><build><plugins><plugin><groupId>org.apache.maven.plugins</groupId><artifactId>maven-compiler-plugin</artifactId><version>3.14.1</version></plugin></plugins></build></project>
'''


def run(command,cwd,env,timeout=600):
    print('RUN',' '.join(map(str,command)),flush=True)
    subprocess.run(command,cwd=cwd,env=env,check=True,timeout=timeout)


def verify(args):
    root=args.root
    assert root.is_absolute() and root.is_dir()
    assert args.jdk.is_absolute() and (args.jdk/'bin/java').is_file()
    assert args.maven.is_absolute() and args.maven.is_file()
    assert args.repository.is_absolute()
    manifest=json.loads((root/'moleapi-generation.json').read_text())
    assert manifest['target']=='protobuf-java' and 'grpc-plugin=true' in manifest['engine']
    assert (root/'fixture/EchoServiceGrpc.java').is_file()
    consumer=root/'runtime-consumer';source=consumer/'src/main/java'
    before={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob('*.java') if 'runtime-consumer' not in p.relative_to(root).parts}
    for path in before:
        destination=source/path;destination.parent.mkdir(parents=True,exist_ok=True)
        destination.write_bytes((root/path).read_bytes())
    entry=source/'fixtureqa/RuntimeFixture.java';entry.parent.mkdir(parents=True,exist_ok=True);entry.write_text(JAVA)
    (consumer/'pom.xml').write_text(POM)
    env=dict(os.environ,JAVA_HOME=str(args.jdk),PATH=str(args.jdk/'bin')+os.pathsep+os.environ.get('PATH',''))
    maven=[str(args.maven),'--batch-mode','--strict-checksums','-Dmaven.repo.local='+str(args.repository)]
    run(maven+['package','org.apache.maven.plugins:maven-dependency-plugin:3.9.0:build-classpath','-Dmdep.outputFile=runtime-classpath.txt'],consumer,env,timeout=900)
    run(maven+['org.apache.maven.plugins:maven-dependency-plugin:3.9.0:tree','-DoutputFile=runtime-dependencies.txt'],consumer,env)
    classpath=str(consumer/'target/classes')+os.pathsep+(consumer/'runtime-classpath.txt').read_text().strip()
    run([str(args.jdk/'bin/java'),'-cp',classpath,'fixtureqa.RuntimeFixture'],consumer,env,timeout=120)
    assert all(hashlib.sha256((root/path).read_bytes()).hexdigest()==digest for path,digest in before.items()),'Generated Java sources changed'
    evidence={'target':'protobuf-java','representation':root.name,'engine':manifest['engine'],'grpc_java':'1.84.1','protobuf_java':'4.36.1','modes':['unary','server','client','bidi'],'descriptor_sha256':hashlib.sha256((root/'descriptor-set.bin').read_bytes()).hexdigest(),'java':subprocess.check_output([str(args.jdk/'bin/java'),'--version'],env=env,text=True).strip(),'maven':subprocess.check_output([str(args.maven),'--version'],env=env,text=True).strip(),'generated_java_sha256':before,'result':'passed'}
    (root/'runtime-evidence.json').write_text(json.dumps(evidence,indent=2)+'\n')
    print(json.dumps(evidence,indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root',type=Path)
    parser.add_argument('--jdk',type=Path,required=True)
    parser.add_argument('--maven',type=Path,required=True)
    parser.add_argument('--repository',type=Path,required=True)
    verify(parser.parse_args())
