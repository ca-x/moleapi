#!/usr/bin/env python3
"""Compile/call original Kotlin coroutine and Java gRPC outputs with explicit QA tools."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

KOTLIN = r'''import com.google.protobuf.ByteString
import com.google.protobuf.Timestamp
import fixture.Common
import fixture.Types.Echo
import fixture.echo
import fixture.EchoServiceGrpcKt
import io.grpc.netty.shaded.io.grpc.netty.NettyChannelBuilder
import io.grpc.netty.shaded.io.grpc.netty.NettyServerBuilder
import java.net.InetSocketAddress
import java.util.Collections
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.flow.collect

fun main() = runBlocking {
    val expected=echo {
        text="鼹鼠 \"quotes\" & +"
        payload=ByteString.copyFrom(byteArrayOf(0,255.toByte(),128.toByte(),65))
        shade=Common.Shade.READY
        labels.addAll(listOf("red","蓝"))
        note="detail"
        observed=Timestamp.newBuilder().setSeconds(1234567).setNanos(123).build()
    }
    val other=expected.toBuilder().setCode(0).build()
    check(expected.detailCase==Echo.DetailCase.NOTE && other.detailCase==Echo.DetailCase.CODE)
    val observations=Collections.synchronizedList(mutableListOf<String>())
    val service=object:EchoServiceGrpcKt.EchoServiceCoroutineImplBase() {
        override suspend fun call(request:Echo):Echo {
            check(request==expected);observations.add("unary");return request
        }
        override fun watch(request:Echo):Flow<Echo> = flow {
            check(request==expected);observations.add("server");emit(request);emit(other)
        }
        override suspend fun collect(requests:Flow<Echo>):Echo {
            val values=requests.toList();check(values==listOf(expected,other));observations.add("client");return values.last()
        }
        override fun chat(requests:Flow<Echo>):Flow<Echo> = flow {
            val values=mutableListOf<Echo>()
            requests.collect { value->values.add(value);emit(value) }
            check(values==listOf(expected,other));observations.add("bidi")
        }
    }
    val server=NettyServerBuilder.forAddress(InetSocketAddress("127.0.0.1",0)).addService(service).build().start()
    val channel=NettyChannelBuilder.forAddress("127.0.0.1",server.port).usePlaintext().build()
    try {
        withTimeout(10000) {
            val client=EchoServiceGrpcKt.EchoServiceCoroutineStub(channel).withDeadlineAfter(10,TimeUnit.SECONDS)
            check(client.call(expected)==expected)
            check(client.watch(expected).toList()==listOf(expected,other))
            check(client.collect(flowOf(expected,other))==other)
            check(client.chat(flowOf(expected,other)).toList()==listOf(expected,other))
            check(observations==listOf("unary","server","client","bidi"))
        }
        println("PASS Kotlin coroutine gRPC unary/server/client/bidi")
    } finally {
        channel.shutdownNow();check(channel.awaitTermination(10,TimeUnit.SECONDS))
        server.shutdownNow();check(server.awaitTermination(10,TimeUnit.SECONDS))
    }
}
'''

BUILD='''plugins { kotlin("jvm") version "2.4.0"; application }
repositories { mavenCentral() }
kotlin { jvmToolchain(21) }
application { mainClass.set("RuntimeFixtureKt") }
dependencies {
    implementation("io.grpc:grpc-netty-shaded:1.84.1")
    implementation("io.grpc:grpc-protobuf:1.84.1")
    implementation("io.grpc:grpc-stub:1.84.1")
    implementation("io.grpc:grpc-kotlin-stub:1.5.0")
    implementation("com.google.protobuf:protobuf-java:4.36.1")
    implementation("com.google.protobuf:protobuf-kotlin:4.36.1")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.10.2")
    implementation("javax.annotation:javax.annotation-api:1.3.2")
}
dependencyLocking { lockAllConfigurations() }
'''


def run(command,cwd,env,timeout=900):
    print('RUN',' '.join(map(str,command)),flush=True)
    subprocess.run(command,cwd=cwd,env=env,check=True,timeout=timeout)


def verify(args):
    root=args.root
    assert root.is_absolute() and root.is_dir()
    assert args.jdk.is_absolute() and (args.jdk/'bin/java').is_file()
    assert args.gradle.is_absolute() and args.gradle.is_file()
    manifest=json.loads((root/'moleapi-generation.json').read_text())
    assert manifest['target']=='protobuf-kotlin' and 'grpc-plugin=true' in manifest['engine']
    assert (root/'fixture/EchoServiceGrpc.java').is_file(),'Missing Java gRPC service dependency'
    before={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob('*') if p.suffix in {'.kt','.java'} and 'runtime-consumer' not in p.relative_to(root).parts}
    consumer=root/'runtime-consumer';consumer.mkdir(exist_ok=True)
    for path in before:
        source=consumer/'src/main'/('kotlin' if path.endswith('.kt') else 'java')/path
        source.parent.mkdir(parents=True,exist_ok=True);source.write_bytes((root/path).read_bytes())
    entry=consumer/'src/main/kotlin/RuntimeFixture.kt';entry.parent.mkdir(parents=True,exist_ok=True);entry.write_text(KOTLIN)
    (consumer/'settings.gradle.kts').write_text('rootProject.name="RuntimeFixture"\n')
    (consumer/'build.gradle.kts').write_text(BUILD)
    env=dict(os.environ,JAVA_HOME=str(args.jdk),PATH=str(args.jdk/'bin')+os.pathsep+os.environ.get('PATH',''))
    command=[str(args.gradle),'--no-daemon','--max-workers=2','--console=plain','-Dorg.gradle.java.installations.paths='+str(args.jdk),'-Dorg.gradle.java.installations.auto-download=false']
    run(command+['build','run','--write-locks'],consumer,env)
    dependencies=subprocess.check_output(command+['dependencies','--configuration','runtimeClasspath'],cwd=consumer,env=env,text=True,timeout=300)
    (consumer/'runtime-dependencies.txt').write_text(dependencies)
    assert all(hashlib.sha256((root/path).read_bytes()).hexdigest()==digest for path,digest in before.items()),'Generated Kotlin/Java sources changed'
    evidence={'target':'protobuf-kotlin','representation':root.name,'engine':manifest['engine'],'grpc_kotlin':'1.5.0','grpc_java':'1.84.1','protobuf':'4.36.1','coroutines':'1.10.2','java':subprocess.check_output([str(args.jdk/'bin/java'),'--version'],env=env,text=True).strip(),'gradle':subprocess.check_output([str(args.gradle),'--version'],env=env,text=True).strip(),'modes':['unary','server','client','bidi'],'descriptor_sha256':hashlib.sha256((root/'descriptor-set.bin').read_bytes()).hexdigest(),'generated_source_sha256':before,'lock_sha256':hashlib.sha256((consumer/'gradle.lockfile').read_bytes()).hexdigest(),'result':'passed'}
    (root/'runtime-evidence.json').write_text(json.dumps(evidence,indent=2)+'\n')
    print(json.dumps(evidence,indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root',type=Path)
    parser.add_argument('--jdk',type=Path,required=True)
    parser.add_argument('--gradle',type=Path,required=True)
    verify(parser.parse_args())
