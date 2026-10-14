#!/usr/bin/env python3
"""Call original PHP gRPC clients against an actual generated Python fixture server."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import grpc

PHP = r'''<?php
require __DIR__.'/vendor/autoload.php';
use Fixture\PBEcho;
use Fixture\EchoServiceClient;
use Fixture\Shade;
use Google\Protobuf\Timestamp;
function ensureFixture($value,$message) { if (!$value) throw new RuntimeException($message); }
function sameFixture(PBEcho $a,PBEcho $b) { return $a->serializeToString()===$b->serializeToString(); }
$expected=new PBEcho(['text'=>'鼹鼠 "quotes" & +','payload'=>"\x00\xff\x80A",'shade'=>Shade::READY,'labels'=>['red','蓝'],'note'=>'detail','observed'=>new Timestamp(['seconds'=>1234567,'nanos'=>123])]);
$other=new PBEcho();$other->mergeFromString($expected->serializeToString());$other->setCode(0);
ensureFixture($expected->getDetail()==='note' && $other->getDetail()==='code','Oneof zero presence lost');
$client=new EchoServiceClient(getenv('FIXTURE_URL'),['credentials'=>Grpc\ChannelCredentials::createInsecure()]);
$options=['timeout'=>10000000];
try {
  [$reply,$status]=$client->Call($expected,[],$options)->wait();ensureFixture($status->code===Grpc\STATUS_OK && $reply instanceof PBEcho && sameFixture($reply,$expected),'Unary response lost');
  $watch=$client->Watch($expected,[],$options);$values=iterator_to_array($watch->responses());
  ensureFixture($watch->getStatus()->code===Grpc\STATUS_OK && count($values)===2 && sameFixture($values[0],$expected) && sameFixture($values[1],$other),'Server-stream response lost');
  $collect=$client->Collect([],$options);$collect->write($expected);$collect->write($other);[$reply,$status]=$collect->wait();
  ensureFixture($status->code===Grpc\STATUS_OK && $reply instanceof PBEcho && sameFixture($reply,$other),'Client-stream response lost');
  $chat=$client->Chat([],$options);$chat->write($expected);$chat->write($other);$chat->writesDone();$values=[];
  while (($value=$chat->read())!==null) $values[]=$value;
  ensureFixture($chat->getStatus()->code===Grpc\STATUS_OK && count($values)===2 && sameFixture($values[0],$expected) && sameFixture($values[1],$other),'Bidi response lost');
  echo json_encode(['php'=>PHP_VERSION,'extension'=>phpversion('grpc'),'modes'=>['unary','server','client','bidi'],'result'=>'passed'],JSON_THROW_ON_ERROR).PHP_EOL;
} finally { $client->close(); }
'''


def run(command,cwd,env,timeout=600):
    print('RUN',' '.join(map(str,command)),flush=True)
    subprocess.run(command,cwd=cwd,env=env,check=True,timeout=timeout)


def verify(args):
    root=args.root
    assert root.is_absolute() and root.is_dir()
    assert args.php.is_absolute() and args.php.is_file()
    assert args.composer.is_absolute() and args.composer.is_file()
    assert args.extension.is_absolute() and args.extension.is_file()
    assert args.library_dir.is_absolute() and args.library_dir.is_dir()
    assert args.server_dir.is_absolute() and args.server_dir.is_dir()
    descriptor=hashlib.sha256((root/'descriptor-set.bin').read_bytes()).hexdigest()
    assert descriptor==hashlib.sha256((args.server_dir/'descriptor-set.bin').read_bytes()).hexdigest(),'Server/client schema differs'
    manifest=json.loads((root/'moleapi-generation.json').read_text())
    assert manifest['target']=='protobuf-php' and 'grpc-plugin=true' in manifest['engine']
    before={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob('*.php') if 'runtime-consumer' not in p.relative_to(root).parts}
    consumer=root/'runtime-consumer';consumer.mkdir(exist_ok=True)
    config={'name':'fixtureqa/runtime','require':{'grpc/grpc':'1.82.0','google/protobuf':'5.36.1'},'autoload':{'psr-4':{'Fixture\\':str(root/'Fixture')+'/','GPBMetadata\\':str(root/'GPBMetadata')+'/'}}}
    (consumer/'composer.json').write_text(json.dumps(config,indent=2)+'\n');(consumer/'runtime_fixture.php').write_text(PHP)
    env=dict(os.environ,LD_LIBRARY_PATH=str(args.library_dir),COMPOSER_HOME=str(consumer/'composer-home'))
    php=[str(args.php),'-d','extension='+str(args.extension)]
    run(php+[str(args.composer),'install','--no-interaction','--no-scripts','--no-plugins','--prefer-dist'],consumer,env)
    for path in before: run(php+['-l',str(root/path)],consumer,env,timeout=30)
    sys.path.insert(0,str(args.server_dir))
    from models import types_pb2,common_pb2
    import fixture_pb2_grpc
    expected=types_pb2.Echo(text='鼹鼠 "quotes" & +',payload=b'\x00\xff\x80A',shade=common_pb2.READY,labels=['red','蓝'],note='detail')
    expected.observed.seconds=1234567;expected.observed.nanos=123
    other=types_pb2.Echo();other.CopyFrom(expected);other.code=0
    observations=[]
    class Echo(fixture_pb2_grpc.EchoServiceServicer):
        def Call(self,request,context):
            assert request==expected;observations.append('unary');return request
        def Watch(self,request,context):
            assert request==expected;observations.append('server');yield request;yield other
        def Collect(self,requests,context):
            values=list(requests);assert values==[expected,other];observations.append('client');return values[-1]
        def Chat(self,requests,context):
            values=[]
            for request in requests: values.append(request);yield request
            assert values==[expected,other];observations.append('bidi')
    server=grpc.server(ThreadPoolExecutor(max_workers=2));fixture_pb2_grpc.add_EchoServiceServicer_to_server(Echo(),server)
    port=server.add_insecure_port('127.0.0.1:0');assert port;server.start()
    try:
        result=subprocess.check_output(php+['runtime_fixture.php'],cwd=consumer,env=dict(env,FIXTURE_URL=f'127.0.0.1:{port}'),text=True,timeout=120)
        print(result)
        records=[json.loads(line) for line in result.splitlines() if line.startswith('{')]
        assert len(records)==1 and records[0]['result']=='passed'
        assert observations==['unary','server','client','bidi'],observations
    finally: server.stop(0).wait(timeout=10)
    dependencies=subprocess.check_output(php+[str(args.composer),'show','--format=json'],cwd=consumer,env=env,text=True)
    (consumer/'runtime-dependencies.json').write_text(dependencies)
    assert all(hashlib.sha256((root/path).read_bytes()).hexdigest()==digest for path,digest in before.items()),'Generated PHP sources changed'
    evidence=dict(records[0],target='protobuf-php',representation=root.name,engine=manifest['engine'],python_server_grpc=grpc.__version__,descriptor_sha256=descriptor,generated_source_sha256=before,lock_sha256=hashlib.sha256((consumer/'composer.lock').read_bytes()).hexdigest())
    (root/'runtime-evidence.json').write_text(json.dumps(evidence,indent=2)+'\n');print(json.dumps(evidence,indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root',type=Path)
    parser.add_argument('--php',type=Path,required=True)
    parser.add_argument('--composer',type=Path,required=True)
    parser.add_argument('--extension',type=Path,required=True)
    parser.add_argument('--library-dir',type=Path,required=True)
    parser.add_argument('--server-dir',type=Path,required=True)
    verify(parser.parse_args())
