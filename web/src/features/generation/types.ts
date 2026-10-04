export interface SnippetClient {client:string;title:string}
export interface SnippetTarget {target:string;title:string;clients:SnippetClient[]}
export interface SnippetCatalog {engine:string;targets:SnippetTarget[];validation:string;scope:string}
export interface Snippet {engine:string;target:string;client:string;code:string;warnings:string[];include_secrets:boolean}
