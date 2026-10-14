#!/usr/bin/env python3
"""Compile/call the original native Tonic SDK with an isolated Rust consumer."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

RUST = r'''use moleapi_grpc::fixture::{Echo, Shade, echo, echo_service_client::EchoServiceClient, echo_service_server::{EchoService, EchoServiceServer}};
use std::{pin::Pin, sync::{Arc,Mutex},time::Duration};
use tokio::sync::{mpsc,oneshot};
use tokio_stream::{Stream,wrappers::{ReceiverStream,TcpListenerStream}};
use tonic::{Request,Response,Status};
type Responses=Pin<Box<dyn Stream<Item=Result<Echo,Status>>+Send>>;
#[derive(Clone)]
struct Service {expected:Echo,other:Echo,observations:Arc<Mutex<Vec<&'static str>>>}
#[tonic::async_trait]
impl EchoService for Service {
    async fn call(&self,request:Request<Echo>)->Result<Response<Echo>,Status> {
        assert_eq!(request.get_ref(),&self.expected);
        self.observations.lock().unwrap().push("unary");
        Ok(Response::new(request.into_inner()))
    }
    type WatchStream=Responses;
    async fn watch(&self,request:Request<Echo>)->Result<Response<Self::WatchStream>,Status> {
        assert_eq!(request.get_ref(),&self.expected);
        self.observations.lock().unwrap().push("server");
        Ok(Response::new(Box::pin(tokio_stream::iter([Ok(self.expected.clone()),Ok(self.other.clone())]))))
    }
    async fn collect(&self,request:Request<tonic::Streaming<Echo>>)->Result<Response<Echo>,Status> {
        let mut stream=request.into_inner();
        assert_eq!(stream.message().await?,Some(self.expected.clone()));
        assert_eq!(stream.message().await?,Some(self.other.clone()));
        assert_eq!(stream.message().await?,None);
        self.observations.lock().unwrap().push("client");
        Ok(Response::new(self.other.clone()))
    }
    type ChatStream=Responses;
    async fn chat(&self,request:Request<tonic::Streaming<Echo>>)->Result<Response<Self::ChatStream>,Status> {
        let mut input=request.into_inner();let expected=self.expected.clone();let other=self.other.clone();let observations=self.observations.clone();
        let (sender,receiver)=mpsc::channel(2);
        tokio::spawn(async move {
            let mut values=Vec::new();
            loop {
                match input.message().await {
                    Ok(Some(value))=>{values.push(value.clone());if sender.send(Ok(value)).await.is_err(){return}},
                    Ok(None)=>break,
                    Err(error)=>{let _=sender.send(Err(error)).await;return},
                }
            }
            assert_eq!(values,vec![expected,other]);observations.lock().unwrap().push("bidi");
        });
        Ok(Response::new(Box::pin(ReceiverStream::new(receiver))))
    }
}
#[tokio::main]
async fn main()->Result<(),Box<dyn std::error::Error>> {
    let expected=Echo{text:"鼹鼠 \"quotes\" & +".into(),payload:vec![0,255,128,65],shade:Shade::Ready as i32,labels:vec!["red".into(),"蓝".into()],detail:Some(echo::Detail::Note("detail".into())),observed:Some(prost_types::Timestamp{seconds:1234567,nanos:123})};
    let mut other=expected.clone();other.detail=Some(echo::Detail::Code(0));
    assert!(matches!(expected.detail,Some(echo::Detail::Note(_))) && matches!(other.detail,Some(echo::Detail::Code(0))));
    let observations=Arc::new(Mutex::new(Vec::new()));
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await?;let address=listener.local_addr()?;
    let (shutdown_sender,shutdown_receiver)=oneshot::channel();
    let server=tokio::spawn(tonic::transport::Server::builder().add_service(EchoServiceServer::new(Service{expected:expected.clone(),other:other.clone(),observations:observations.clone()})).serve_with_incoming_shutdown(TcpListenerStream::new(listener),async {let _=shutdown_receiver.await;}));
    tokio::time::timeout(Duration::from_secs(10),async {
        let mut client=EchoServiceClient::connect(format!("http://{address}")).await?;
        assert_eq!(client.call(expected.clone()).await?.into_inner(),expected);
        let mut watched=client.watch(expected.clone()).await?.into_inner();
        assert_eq!(watched.message().await?,Some(expected.clone()));assert_eq!(watched.message().await?,Some(other.clone()));assert_eq!(watched.message().await?,None);
        let collected=client.collect(tokio_stream::iter([expected.clone(),other.clone()])).await?.into_inner();assert_eq!(collected,other);
        let mut chatted=client.chat(tokio_stream::iter([expected.clone(),other.clone()])).await?.into_inner();
        assert_eq!(chatted.message().await?,Some(expected.clone()));assert_eq!(chatted.message().await?,Some(other.clone()));assert_eq!(chatted.message().await?,None);
        assert_eq!(*observations.lock().unwrap(),vec!["unary","server","client","bidi"]);
        Ok::<_,Box<dyn std::error::Error>>(())
    }).await??;
    shutdown_sender.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(10),server).await???;
    println!("PASS Rust Tonic unary/server/client/bidi");Ok(())
}
'''

MANIFEST='''[package]
name="moleapi_tonic_fixture"
version="0.1.0"
edition="2021"
[workspace]
[dependencies]
moleapi_grpc={path=".."}
prost-types="0.14"
tonic={version="0.14",features=["transport"]}
tokio={version="1",features=["full"]}
tokio-stream={version="0.1",features=["net"]}
'''


def run(command,cwd,timeout=900):
    print('RUN',' '.join(map(str,command)),flush=True)
    subprocess.run(command,cwd=cwd,check=True,timeout=timeout)


def verify(args):
    root=args.root
    assert root.is_absolute() and root.is_dir()
    assert args.cargo.is_absolute() and args.cargo.is_file()
    assert args.target_dir.is_absolute()
    manifest=json.loads((root/'moleapi-generation.json').read_text())
    assert manifest['target']=='rust-tonic' and manifest['engine']=='tonic-prost-build@0.14.6'
    before={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [root/'Cargo.toml',*list((root/'src').rglob('*.rs'))]}
    consumer=root/'runtime-consumer';(consumer/'src').mkdir(parents=True,exist_ok=True)
    (consumer/'Cargo.toml').write_text(MANIFEST);(consumer/'src/main.rs').write_text(RUST)
    run([str(args.cargo),'build','--manifest-path',str(consumer/'Cargo.toml'),'--target-dir',str(args.target_dir)],consumer)
    dependencies=subprocess.check_output([str(args.cargo),'tree','--manifest-path',str(consumer/'Cargo.toml'),'--locked'],cwd=consumer,text=True)
    (consumer/'runtime-dependencies.txt').write_text(dependencies)
    run([str(args.target_dir/'debug'/('moleapi_tonic_fixture.exe' if os.name=='nt' else 'moleapi_tonic_fixture'))],consumer,timeout=120)
    assert all(hashlib.sha256((root/path).read_bytes()).hexdigest()==digest for path,digest in before.items()),'Generated SDK source changed'
    evidence={'target':'rust-tonic','representation':root.name,'engine':manifest['engine'],'cargo':subprocess.check_output([str(args.cargo),'--version'],text=True).strip(),'modes':['unary','server','client','bidi'],'descriptor_sha256':hashlib.sha256((root/'descriptor-set.bin').read_bytes()).hexdigest(),'consumer_lock_sha256':hashlib.sha256((consumer/'Cargo.lock').read_bytes()).hexdigest(),'generated_source_sha256':before,'result':'passed'}
    (root/'runtime-evidence.json').write_text(json.dumps(evidence,indent=2)+'\n')
    print(json.dumps(evidence,indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root',type=Path)
    parser.add_argument('--cargo',type=Path,required=True)
    parser.add_argument('--target-dir',type=Path,required=True)
    verify(parser.parse_args())
