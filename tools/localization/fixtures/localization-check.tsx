import { Editor } from "./shared/ui";
import ReactDOM from "react-dom/client";
import { Component, useState } from "react";
import { Theme } from "@radix-ui/themes";
import "@radix-ui/themes/styles.css";
import "./styles.css";
import GraphQLWorkbench from "./features/graphql/GraphQLWorkbench";
import { WorkbenchContext } from "./features/workbench/context";
import type { useWorkbenchController } from "./features/workbench/useWorkbenchController";
import { initialData, newRequest } from "./shared/model";
import LanguageSelector from "./shared/i18n/LanguageSelector";
import { setLanguage } from "./shared/i18n";
const schema={id:"schema",name:"原文 Schema",kind:"graphql-sdl" as const,source:'"原文描述" type Query { hello: String }'};
window.fetch=async()=>new Response(JSON.stringify({sdl:schema.source}),{headers:{"Content-Type":"application/json"}});
const data={...initialData(),specifications:[schema]};
function Check(){
 const [request,setRequest]=useState({...newRequest("我的 GraphQL","https://example.com/graphql"),id:"request",specification_id:"schema",protocol:{kind:"graphql" as const,document:'query First { hello }\nquery Second { hello } # 原文',variables:{},variables_source:'{"原文":"值"}',operation_name:"Second"}});
 const state={request,updateRequest:(patch:object)=>setRequest(value=>({...value,...patch})),draft:{id:"workspace",data},accountId:"owner",authenticated:true,dark:false,graphqlRun:{current:null},localVariables:{values:()=>[],apply:()=>{}},updateData:()=>{},save:async()=>true,dirty:false,protocolSession:{}};
 (window as unknown as {localizationCheck:unknown}).localizationCheck={request,setLanguage,state};
 return <Theme><LanguageSelector/><Editor label="CodeMirror" value="// 原文代码\nconst value = 1" dark={false}/><div style={{height:"850px"}}><WorkbenchContext.Provider value={state as unknown as ReturnType<typeof useWorkbenchController>}><GraphQLWorkbench/></WorkbenchContext.Provider></div></Theme>
}
class Boundary extends Component<{children:React.ReactNode},{error:string}> {
 state={error:""};
 componentDidCatch(error:Error,info:React.ErrorInfo){ (window as unknown as {checkError:unknown}).checkError={message:error.message,stack:info.componentStack};this.setState({error:error.message}); }
 render(){return this.state.error?<pre>{this.state.error}</pre>:this.props.children;}
}
ReactDOM.createRoot(document.getElementById('root')!).render(<Boundary><Check/></Boundary>);
