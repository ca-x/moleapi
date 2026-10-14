#!/usr/bin/env python3
"""Compile/call original official Go protobuf/gRPC plugin output using real -race tests."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

GO=r'''package fixtureqa
import (
 "context"
 "errors"
 "io"
 "net"
 "slices"
 "sync"
 "testing"
 "time"
 pb "example.invalid/moleapi/fixture"
 "google.golang.org/grpc"
 "google.golang.org/grpc/codes"
 "google.golang.org/grpc/credentials/insecure"
 "google.golang.org/grpc/status"
 "google.golang.org/protobuf/proto"
 "google.golang.org/protobuf/types/known/timestamppb"
)
type service struct {
 pb.UnimplementedEchoServiceServer
 expected,other *pb.Echo
 mutex sync.Mutex
 observed []string
}
func (s *service) record(mode string) {s.mutex.Lock();defer s.mutex.Unlock();s.observed=append(s.observed,mode)}
func (s *service) Call(_ context.Context, request *pb.Echo)(*pb.Echo,error){
 if !proto.Equal(request,s.expected){return nil,status.Error(codes.InvalidArgument,"unary request lost")};s.record("unary");return request,nil
}
func (s *service) Watch(request *pb.Echo,output grpc.ServerStreamingServer[pb.Echo])error{
 if !proto.Equal(request,s.expected){return status.Error(codes.InvalidArgument,"server-stream request lost")};s.record("server")
 if err:=output.Send(request);err!=nil{return err};return output.Send(s.other)
}
func (s *service) Collect(input grpc.ClientStreamingServer[pb.Echo,pb.Echo])error{
 var values []*pb.Echo
 for {value,err:=input.Recv();if errors.Is(err,io.EOF){break};if err!=nil{return err};values=append(values,value)}
 if len(values)!=2||!proto.Equal(values[0],s.expected)||!proto.Equal(values[1],s.other){return status.Error(codes.InvalidArgument,"client-stream request lost")};s.record("client");return input.SendAndClose(s.other)
}
func (s *service) Chat(stream grpc.BidiStreamingServer[pb.Echo,pb.Echo])error{
 var values []*pb.Echo
 for {value,err:=stream.Recv();if errors.Is(err,io.EOF){break};if err!=nil{return err};values=append(values,value);if err=stream.Send(value);err!=nil{return err}}
 if len(values)!=2||!proto.Equal(values[0],s.expected)||!proto.Equal(values[1],s.other){return status.Error(codes.InvalidArgument,"bidi request lost")};s.record("bidi");return nil
}
func TestRuntimeFixture(t *testing.T){
 expected:=new(pb.Echo{Text:"鼹鼠 \"quotes\" & +",Payload:[]byte{0,255,128,65},Shade:pb.Shade_READY,Labels:[]string{"red","蓝"},Detail:new(pb.Echo_Note{Note:"detail"}),Observed:new(timestamppb.Timestamp{Seconds:1234567,Nanos:123})})
 other:=proto.Clone(expected).(*pb.Echo);other.Detail=new(pb.Echo_Code{Code:0})
 if _,ok:=other.Detail.(*pb.Echo_Code);!ok{t.Fatal("oneof zero presence lost")}
 listener,err:=net.Listen("tcp","127.0.0.1:0");if err!=nil{t.Fatal(err)}
 implementation:=new(service{expected:expected,other:other})
 server:=grpc.NewServer();pb.RegisterEchoServiceServer(server,implementation)
 var serving sync.WaitGroup;done:=make(chan error,1);serving.Go(func(){done<-server.Serve(listener)})
 t.Cleanup(func(){server.Stop();serving.Wait();if err:=<-done;err!=nil&&!errors.Is(err,grpc.ErrServerStopped){t.Error(err)}})
 connection,err:=grpc.NewClient(listener.Addr().String(),grpc.WithTransportCredentials(insecure.NewCredentials()));if err!=nil{t.Fatal(err)}
 t.Cleanup(func(){if err:=connection.Close();err!=nil{t.Error(err)}})
 client:=pb.NewEchoServiceClient(connection);ctx,cancel:=context.WithTimeout(t.Context(),10*time.Second);defer cancel()
 reply,err:=client.Call(ctx,expected);if err!=nil||!proto.Equal(reply,expected){t.Fatalf("unary response lost: %v",err)}
 watch,err:=client.Watch(ctx,expected);if err!=nil{t.Fatal(err)}
 var watched []*pb.Echo;for {value,err:=watch.Recv();if errors.Is(err,io.EOF){break};if err!=nil{t.Fatal(err)};watched=append(watched,value)}
 if len(watched)!=2||!proto.Equal(watched[0],expected)||!proto.Equal(watched[1],other){t.Fatal("server-stream response lost")}
 collect,err:=client.Collect(ctx);if err!=nil{t.Fatal(err)};if err=collect.Send(expected);err!=nil{t.Fatal(err)};if err=collect.Send(other);err!=nil{t.Fatal(err)}
 reply,err=collect.CloseAndRecv();if err!=nil||!proto.Equal(reply,other){t.Fatalf("client-stream response lost: %v",err)}
 chat,err:=client.Chat(ctx);if err!=nil{t.Fatal(err)};if err=chat.Send(expected);err!=nil{t.Fatal(err)};if err=chat.Send(other);err!=nil{t.Fatal(err)};if err=chat.CloseSend();err!=nil{t.Fatal(err)}
 var chatted []*pb.Echo;for {value,err:=chat.Recv();if errors.Is(err,io.EOF){break};if err!=nil{t.Fatal(err)};chatted=append(chatted,value)}
 if len(chatted)!=2||!proto.Equal(chatted[0],expected)||!proto.Equal(chatted[1],other){t.Fatal("bidi response lost")}
 implementation.mutex.Lock();observed:=slices.Clone(implementation.observed);implementation.mutex.Unlock()
 if !slices.Equal(observed,[]string{"unary","server","client","bidi"}){t.Fatalf("wrong modes: %v",observed)}
}
'''

MOD='''module example.invalid/moleapi

go 1.27.0

require (
 google.golang.org/grpc v1.84.0
 google.golang.org/protobuf v1.36.12
)
'''


def run(command,cwd,env,timeout=900):
    print('RUN',' '.join(map(str,command)),flush=True)
    subprocess.run(command,cwd=cwd,env=env,check=True,timeout=timeout)


def verify(args):
    root=args.root
    assert root.is_absolute() and root.is_dir()
    assert args.go.is_absolute() and args.go.is_file()
    assert args.cache.is_absolute()
    manifest=json.loads((root/'moleapi-generation.json').read_text())
    assert manifest['target']=='protobuf-go' and 'grpc-plugin=true' in manifest['engine']
    before={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob('*.go') if 'runtime-consumer' not in p.relative_to(root).parts}
    assert 'example.invalid/moleapi/fixture/fixture_grpc.pb.go' in before
    consumer=root/'runtime-consumer';consumer.mkdir(exist_ok=True)
    for path in before:
        relative=Path(path).relative_to('example.invalid/moleapi')
        destination=consumer/relative;destination.parent.mkdir(parents=True,exist_ok=True);destination.write_bytes((root/path).read_bytes())
    (consumer/'go.mod').write_text(MOD);(consumer/'runtime_fixture_test.go').write_text(GO)
    env=dict(os.environ,GOPATH=str(args.cache/'gopath'),GOMODCACHE=str(args.cache/'modcache'),GOSUMDB='sum.golang.org')
    formatter=args.go.parent/'gofmt';assert formatter.is_file()
    run([str(formatter),'-w',str(consumer/'runtime_fixture_test.go')],consumer,env)
    run([str(args.go),'mod','tidy'],consumer,env)
    run([str(args.go),'mod','verify'],consumer,env)
    run([str(args.go),'test','-race','-count=1','-v','./...'],consumer,env)
    dependencies=subprocess.check_output([str(args.go),'list','-m','-json','all'],cwd=consumer,env=env,text=True)
    (consumer/'runtime-dependencies.json').write_text(dependencies)
    assert all(hashlib.sha256((root/path).read_bytes()).hexdigest()==digest for path,digest in before.items()),'Generated Go source changed'
    evidence={'target':'protobuf-go','representation':root.name,'engine':manifest['engine'],'go':subprocess.check_output([str(args.go),'version'],text=True).strip(),'grpc':'v1.84.0','protobuf':'v1.36.12','modes':['unary','server','client','bidi'],'race':True,'descriptor_sha256':hashlib.sha256((root/'descriptor-set.bin').read_bytes()).hexdigest(),'generated_source_sha256':before,'go_sum_sha256':hashlib.sha256((consumer/'go.sum').read_bytes()).hexdigest(),'result':'passed'}
    (root/'runtime-evidence.json').write_text(json.dumps(evidence,indent=2)+'\n');print(json.dumps(evidence,indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root',type=Path)
    parser.add_argument('--go',type=Path,required=True)
    parser.add_argument('--cache',type=Path,required=True)
    verify(parser.parse_args())
