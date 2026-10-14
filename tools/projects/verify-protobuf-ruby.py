#!/usr/bin/env python3
"""Import/call original generated Ruby protobuf/gRPC code with explicit QA runtime."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

RUBY = r'''require 'json'
require 'grpc'
require 'fixture_services_pb'
def ensure_fixture(value,message)
  raise message unless value
end
class RuntimeService < Fixture::EchoService::Service
  def initialize(expected,other,observations,lock)
    @expected=expected;@other=other;@observations=observations;@lock=lock
  end
  def observed(mode)
    @lock.synchronize { @observations << mode }
  end
  def call(request,_call)
    ensure_fixture(request==@expected,'Unary request lost');observed('unary');request
  end
  def watch(request,_call)
    ensure_fixture(request==@expected,'Server-stream request lost');observed('server')
    [request,@other].each
  end
  def collect(call)
    values=call.each_remote_read.to_a
    ensure_fixture(values==[@expected,@other],'Client-stream request lost');observed('client');values.last
  end
  def chat(requests)
    Enumerator.new do |output|
      values=[]
      requests.each { |value| values << value;output << value }
      ensure_fixture(values==[@expected,@other],'Bidi request lost');observed('bidi')
    end
  end
end
expected=Fixture::Echo.new(text:'鼹鼠 "quotes" & +',payload:[0,255,128,65].pack('C*'),shade: :READY,labels:['red','蓝'],note:'detail',observed:Google::Protobuf::Timestamp.new(seconds:1234567,nanos:123))
other=Google::Protobuf.deep_copy(expected);other.code=0
ensure_fixture(expected.detail==:note && other.detail==:code,'Oneof zero presence lost')
observations=[];lock=Mutex.new
server=GRPC::RpcServer.new(pool_size:2)
port=server.add_http2_port('127.0.0.1:0',:this_port_is_insecure)
ensure_fixture(port>0,'Missing loopback port')
server.handle(RuntimeService.new(expected,other,observations,lock))
thread=Thread.new { server.run_till_terminated }
channel=nil
begin
  server.wait_till_running
  address="127.0.0.1:#{port}"
  channel=GRPC::Core::Channel.new(address,{},:this_channel_is_insecure)
  stub=Fixture::EchoService::Stub.new(address,:this_channel_is_insecure,channel_override:channel)
  ensure_fixture(stub.call(expected,deadline:Time.now+10)==expected,'Unary response lost')
  ensure_fixture(stub.watch(expected,deadline:Time.now+10).to_a==[expected,other],'Server-stream response lost')
  ensure_fixture(stub.collect([expected,other].each,deadline:Time.now+10)==other,'Client-stream response lost')
  ensure_fixture(stub.chat([expected,other].each,deadline:Time.now+10).to_a==[expected,other],'Bidi response lost')
  ensure_fixture(observations==['unary','server','client','bidi'],'RPC mode/order lost')
  puts JSON.generate({ruby:RUBY_DESCRIPTION,grpc:Gem.loaded_specs['grpc'].version.to_s,protobuf:Gem.loaded_specs['google-protobuf'].version.to_s,gems:Gem.loaded_specs.transform_values { |spec| spec.version.to_s },modes:observations,result:'passed'})
ensure
  channel.close if channel
  server.stop
  ensure_fixture(!thread.join(10).nil?,'Server thread cleanup failed')
end
'''


def verify(args):
    root=args.root
    assert root.is_absolute() and root.is_dir()
    assert args.ruby.is_absolute() and args.ruby.is_file()
    assert args.gem_home.is_absolute() and args.gem_home.is_dir()
    manifest=json.loads((root/'moleapi-generation.json').read_text())
    assert manifest['target']=='protobuf-ruby' and 'grpc-plugin=true' in manifest['engine']
    before={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob('*.rb') if p.name!='runtime_fixture.rb'}
    entry=root/'runtime_fixture.rb';entry.write_text(RUBY)
    default_gems=subprocess.check_output([str(args.ruby),'-rrubygems','-e','print Gem.default_dir'],text=True).strip()
    assert Path(default_gems).is_relative_to(args.ruby.parent.parent), 'Use only the selected Ruby runtime gems'
    env=dict(os.environ,GEM_HOME=str(args.gem_home),GEM_PATH=str(args.gem_home)+os.pathsep+default_gems)
    for path in before:
        subprocess.run([str(args.ruby),'-c',str(root/path)],env=env,check=True,timeout=30)
    print('RUN',str(args.ruby),'-I',str(root),str(entry),flush=True)
    result=subprocess.check_output([str(args.ruby),'-I',str(root),str(entry)],cwd=root,env=env,text=True,timeout=120)
    print(result)
    records=[json.loads(line) for line in result.splitlines() if line.startswith('{')]
    assert len(records)==1 and records[0]['result']=='passed'
    assert all(hashlib.sha256((root/path).read_bytes()).hexdigest()==digest for path,digest in before.items()),'Generated Ruby source changed'
    evidence=dict(records[0],target='protobuf-ruby',representation=root.name,engine=manifest['engine'],descriptor_sha256=hashlib.sha256((root/'descriptor-set.bin').read_bytes()).hexdigest(),generated_source_sha256=before)
    (root/'runtime-evidence.json').write_text(json.dumps(evidence,indent=2)+'\n')
    print(json.dumps(evidence,indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root',type=Path)
    parser.add_argument('--ruby',type=Path,required=True)
    parser.add_argument('--gem-home',type=Path,required=True)
    verify(parser.parse_args())
