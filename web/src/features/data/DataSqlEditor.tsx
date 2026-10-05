import {useMemo,useRef,useEffect} from "react";
import CodeMirror,{type ReactCodeMirrorRef} from "@uiw/react-codemirror";
import {HighlightStyle,syntaxHighlighting} from "@codemirror/language";
import {tags} from "@lezer/highlight";
import {linter,lintGutter} from "@codemirror/lint";
import {completionSchema,syntaxDiagnostics} from "./editor";
import {sql} from "@codemirror/lang-sql";
import {EditorState,Compartment,Prec} from "@codemirror/state";
import {EditorView,keymap} from "@codemirror/view";
import {t,useLanguage} from "../../shared/i18n";
import phrases from "../../shared/i18n/codemirror-zh-CN.json";
import type {DataSource,SchemaTable} from "./types";
import {dialect} from "./model";
export function DataSqlEditor({value,change,source,tables,dark,selection,run}:{value:string;change:(sql:string)=>void;source:DataSource;tables:SchemaTable[];dark:boolean;selection:(from:number,to:number)=>void;run:()=>void}) {
  const {language}=useLanguage();const ref=useRef<ReactCodeMirrorRef>(null),locale=useMemo(()=>new Compartment(),[]),attributes=useMemo(()=>new Compartment(),[]);
  const latest=useRef({change,selection,run});latest.current={change,selection,run};
  const schema=useMemo(()=>completionSchema(tables),[tables]);
  const extensions=useMemo(()=>[Prec.highest(keymap.of([{key:"Mod-Enter",run:()=>{latest.current.run();return true;}}])),syntaxHighlighting(HighlightStyle.define([{tag:tags.typeName,class:"cm-data-type"},{tag:tags.keyword,class:"cm-data-keyword"},{tag:tags.name,class:"cm-data-name"},{tag:tags.comment,class:"cm-data-comment"}])),sql({dialect:dialect(source),schema}),lintGutter(),linter(view=>syntaxDiagnostics(view.state.doc.toString(),source,t("SQL 语法结构有误，请检查括号或引号。"))),locale.of(EditorState.phrases.of({})),attributes.of(EditorView.contentAttributes.of({"aria-label":t("SQL 查询编辑器")})),EditorView.updateListener.of(update=>{if(update.selectionSet||update.docChanged){const selected=update.state.selection.main;latest.current.selection(selected.from,selected.to);}})],[source,schema,locale,attributes,language]);
  useEffect(()=>{const view=ref.current?.view;if(view){view.scrollDOM.tabIndex=0;view.scrollDOM.setAttribute("role","region");view.scrollDOM.setAttribute("aria-label",t("SQL 编辑器滚动区域"));}ref.current?.view?.dispatch({effects:[locale.reconfigure(EditorState.phrases.of(language==="zh-CN"?phrases:{})),attributes.reconfigure(EditorView.contentAttributes.of({"aria-label":t("SQL 查询编辑器")}))]});},[language,locale,attributes]);
  return <CodeMirror onCreateEditor={view=>{view.scrollDOM.tabIndex=0;view.scrollDOM.setAttribute("role","region");view.scrollDOM.setAttribute("aria-label",t("SQL 编辑器滚动区域"));}} ref={ref} className="data-sql-editor" value={value} onChange={value=>latest.current.change(value)} extensions={extensions} theme={dark?"dark":"light"} height="200px" basicSetup={{lineNumbers:true,foldGutter:true,autocompletion:true}}/>;
}
