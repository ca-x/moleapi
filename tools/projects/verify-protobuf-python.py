#!/usr/bin/env python3
"""Call actual generated Python gRPC consumers from trusted imported fixtures."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import importlib.metadata
import json
from pathlib import Path
import sys
import grpc


def verify(root):
    sys.path.insert(0, str(root))
    from models import types_pb2, common_pb2
    import fixture_pb2_grpc

    expected=types_pb2.Echo(text='鼹鼠 "quotes" & +',payload=b'\x00\xff\x80A',shade=common_pb2.READY,labels=['red','蓝'],note='detail')
    expected.observed.seconds=1234567
    expected.observed.nanos=123
    other=types_pb2.Echo()
    other.CopyFrom(expected)
    other.code=0
    assert other.WhichOneof('detail')=='code' and expected.WhichOneof('detail')=='note'
    observations=[]

    class Echo(fixture_pb2_grpc.EchoServiceServicer):
        def Call(self, request, context):
            assert request==expected
            observations.append('unary')
            return request
        def Watch(self, request, context):
            assert request==expected
            observations.append('server')
            yield request
            yield other
        def Collect(self, request_iterator, context):
            requests=list(request_iterator)
            assert requests==[expected,other]
            observations.append('client')
            return requests[-1]
        def Chat(self, request_iterator, context):
            requests=[]
            for request in request_iterator:
                requests.append(request)
                yield request
            assert requests==[expected,other]
            observations.append('bidi')

    server=grpc.server(ThreadPoolExecutor(max_workers=2))
    fixture_pb2_grpc.add_EchoServiceServicer_to_server(Echo(),server)
    port=server.add_insecure_port('127.0.0.1:0')
    assert port
    server.start()
    try:
        with grpc.insecure_channel(f'127.0.0.1:{port}') as channel:
            grpc.channel_ready_future(channel).result(timeout=10)
            stub=fixture_pb2_grpc.EchoServiceStub(channel)
            assert stub.Call(expected,timeout=10)==expected
            assert list(stub.Watch(expected,timeout=10))==[expected,other]
            assert stub.Collect(iter([expected,other]),timeout=10)==other
            assert list(stub.Chat(iter([expected,other]),timeout=10))==[expected,other]
        assert observations==['unary','server','client','bidi'],observations
    finally:
        server.stop(0).wait(timeout=10)
    manifest=json.loads((root/'moleapi-generation.json').read_text())
    assert manifest['target']=='protobuf-python' and 'grpc-plugin=true' in manifest['engine']
    assert not (root/'google/protobuf/timestamp_pb2.py').exists()
    assert not (root/'unused_pb2.py').exists()
    evidence={'target':'protobuf-python','representation':root.name,'engine':manifest['engine'],'grpcio':importlib.metadata.version('grpcio'),'protobuf':importlib.metadata.version('protobuf'),'modes':observations,'descriptor_sha256':hashlib.sha256((root/'descriptor-set.bin').read_bytes()).hexdigest(),'result':'passed'}
    (root/'runtime-evidence.json').write_text(json.dumps(evidence,indent=2)+'\n')
    print(json.dumps(evidence,indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root',type=Path)
    args=parser.parse_args()
    assert args.root.is_absolute() and args.root.is_dir()
    verify(args.root)
