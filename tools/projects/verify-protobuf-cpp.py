#!/usr/bin/env python3
"""Compile/call original generated C++ protobuf/gRPC files with explicit QA prefix."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import subprocess

CPP = r'''#include "fixture.grpc.pb.h"
#include <google/protobuf/util/message_differencer.h>
#include <grpcpp/grpcpp.h>
#include <chrono>
#include <iostream>
#include <mutex>
#include <stdexcept>
#include <string>
#include <vector>
using fixture::Echo;
void check(bool value,const char* message) {if(!value)throw std::runtime_error(message);}
bool same(const Echo& a,const Echo& b) {return google::protobuf::util::MessageDifferencer::Equals(a,b);}
class Service final:public fixture::EchoService::Service {
 public:
  Echo expected,other;std::vector<std::string> observations;std::mutex mutex;
  Service(Echo a,Echo b):expected(std::move(a)),other(std::move(b)){}
  void observed(const char* mode){std::lock_guard<std::mutex> guard(mutex);observations.push_back(mode);}
  grpc::Status Call(grpc::ServerContext*,const Echo* request,Echo* response) override {
    check(same(*request,expected),"Unary request lost");observed("unary");*response=*request;return grpc::Status::OK;
  }
  grpc::Status Watch(grpc::ServerContext*,const Echo* request,grpc::ServerWriter<Echo>* writer) override {
    check(same(*request,expected),"Server-stream request lost");observed("server");
    check(writer->Write(*request)&&writer->Write(other),"Server stream closed early");return grpc::Status::OK;
  }
  grpc::Status Collect(grpc::ServerContext*,grpc::ServerReader<Echo>* reader,Echo* response) override {
    Echo value;std::vector<Echo> values;while(reader->Read(&value))values.push_back(value);
    check(values.size()==2&&same(values[0],expected)&&same(values[1],other),"Client-stream request lost");observed("client");*response=other;return grpc::Status::OK;
  }
  grpc::Status Chat(grpc::ServerContext*,grpc::ServerReaderWriter<Echo,Echo>* stream) override {
    Echo value;std::vector<Echo> values;while(stream->Read(&value)){values.push_back(value);check(stream->Write(value),"Bidi stream closed early");}
    check(values.size()==2&&same(values[0],expected)&&same(values[1],other),"Bidi request lost");observed("bidi");return grpc::Status::OK;
  }
};
void deadline(grpc::ClientContext& context){context.set_deadline(std::chrono::system_clock::now()+std::chrono::seconds(10));}
int main(){
  Echo expected;expected.set_text("鼹鼠 \"quotes\" & +");expected.set_payload(std::string({char(0),char(255),char(128),'A'}));
  expected.set_shade(fixture::READY);expected.add_labels("red");expected.add_labels("蓝");expected.set_note("detail");
  expected.mutable_observed()->set_seconds(1234567);expected.mutable_observed()->set_nanos(123);
  Echo other=expected;other.set_code(0);check(expected.detail_case()==Echo::kNote&&other.detail_case()==Echo::kCode,"Oneof zero presence lost");
  Service service(expected,other);grpc::ServerBuilder builder;int port=0;
  builder.AddListeningPort("127.0.0.1:0",grpc::InsecureServerCredentials(),&port);builder.RegisterService(&service);
  auto server=builder.BuildAndStart();check(server!=nullptr&&port>0,"Server bind failed");
  auto channel=grpc::CreateChannel("127.0.0.1:"+std::to_string(port),grpc::InsecureChannelCredentials());
  check(channel->WaitForConnected(std::chrono::system_clock::now()+std::chrono::seconds(10)),"Channel not ready");
  auto client=fixture::EchoService::NewStub(channel);
  {grpc::ClientContext context;deadline(context);Echo response;check(client->Call(&context,expected,&response).ok()&&same(response,expected),"Unary response lost");}
  {grpc::ClientContext context;deadline(context);auto reader=client->Watch(&context,expected);Echo value;std::vector<Echo> values;while(reader->Read(&value))values.push_back(value);
   check(reader->Finish().ok()&&values.size()==2&&same(values[0],expected)&&same(values[1],other),"Server-stream response lost");}
  {grpc::ClientContext context;deadline(context);Echo response;auto writer=client->Collect(&context,&response);
   check(writer->Write(expected)&&writer->Write(other)&&writer->WritesDone(),"Client stream closed early");check(writer->Finish().ok()&&same(response,other),"Client-stream response lost");}
  {grpc::ClientContext context;deadline(context);auto stream=client->Chat(&context);check(stream->Write(expected)&&stream->Write(other)&&stream->WritesDone(),"Bidi input closed early");Echo value;std::vector<Echo> values;while(stream->Read(&value))values.push_back(value);
   check(stream->Finish().ok()&&values.size()==2&&same(values[0],expected)&&same(values[1],other),"Bidi response lost");}
  {std::lock_guard<std::mutex> guard(service.mutex);check(service.observations==std::vector<std::string>({"unary","server","client","bidi"}),"RPC mode/order lost");}
  client.reset();channel.reset();server->Shutdown(std::chrono::system_clock::now()+std::chrono::seconds(10));server->Wait();
  std::cout<<"PASS C++ gRPC unary/server/client/bidi\n";return 0;
}
'''


def run(command,cwd,env,timeout=900):
    print('RUN',' '.join(map(str,command)),flush=True)
    subprocess.run(command,cwd=cwd,env=env,check=True,timeout=timeout)


def verify(args):
    root=args.root
    assert root.is_absolute() and root.is_dir()
    assert args.cxx.is_absolute() and args.cxx.is_file()
    assert args.pkg_config.is_absolute() and args.pkg_config.is_file()
    assert args.prefix.is_absolute() and (args.prefix/'lib/pkgconfig').is_dir()
    manifest=json.loads((root/'moleapi-generation.json').read_text())
    assert manifest['target']=='protobuf-cpp' and 'grpc-plugin=true' in manifest['engine']
    before={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob('*') if p.suffix in {'.cc','.h'} and 'runtime-consumer' not in p.relative_to(root).parts}
    consumer=root/'runtime-consumer';consumer.mkdir(exist_ok=True)
    entry=consumer/'main.cc';entry.write_text(CPP)
    env=dict(os.environ,PKG_CONFIG_PATH=str(args.prefix/'lib/pkgconfig'),LD_LIBRARY_PATH=str(args.prefix/'lib'))
    pc=[str(args.pkg_config),'--define-prefix']
    flags=shlex.split(subprocess.check_output(pc+['--cflags','--libs','grpc++','protobuf'],env=env,text=True))
    executable=consumer/'runtime_fixture'
    sources=[str(root/path) for path in before if path.endswith('.cc')]
    command=[str(args.cxx),'-std=c++17','-O2','-pthread','-I'+str(root),str(entry),*sources,*flags,'-o',str(executable)]
    (consumer/'runtime-build-command.json').write_text(json.dumps(command,indent=2)+'\n')
    run(command,consumer,env)
    run([str(executable)],consumer,env,timeout=120)
    linkage=subprocess.check_output(['ldd',str(executable)],env=env,text=True)
    assert 'not found' not in linkage,linkage
    (consumer/'runtime-linkage.txt').write_text(linkage)
    assert all(hashlib.sha256((root/path).read_bytes()).hexdigest()==digest for path,digest in before.items()),'Generated C++ sources changed'
    evidence={'target':'protobuf-cpp','representation':root.name,'engine':manifest['engine'],'compiler':subprocess.check_output([str(args.cxx),'--version'],text=True).strip(),'grpc':subprocess.check_output(pc+['--modversion','grpc++'],env=env,text=True).strip(),'protobuf':subprocess.check_output(pc+['--modversion','protobuf'],env=env,text=True).strip(),'modes':['unary','server','client','bidi'],'descriptor_sha256':hashlib.sha256((root/'descriptor-set.bin').read_bytes()).hexdigest(),'generated_source_sha256':before,'result':'passed'}
    (root/'runtime-evidence.json').write_text(json.dumps(evidence,indent=2)+'\n')
    print(json.dumps(evidence,indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root',type=Path)
    parser.add_argument('--cxx',type=Path,required=True)
    parser.add_argument('--pkg-config',type=Path,required=True)
    parser.add_argument('--prefix',type=Path,required=True)
    verify(parser.parse_args())
