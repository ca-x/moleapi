export interface ProjectTarget {id:string;kind:string;upstream_stability:string;validation:string;options?:Record<string,string|boolean|number|null>}
export interface ProjectCatalog {targets:ProjectTarget[];java_available:boolean;protoc_available?:boolean;grpc_plugins?:string[];native_engine:string;multi_language_engine:string;scope:string}
export interface ProjectFile {path:string;encoding:"utf8"|"base64";content:string;bytes:number;sha256:string;executable:boolean}
export interface ProjectArtifact {engine:string;target:string;source_sha256:string;options:Record<string,unknown>;include_secrets:boolean;files:ProjectFile[];archive_base64:string}
