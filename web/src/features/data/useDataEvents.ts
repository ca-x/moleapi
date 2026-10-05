import { useEffect,useRef,useState } from "react";
import type { ProtocolEvent } from "../protocols/types";
import type { ConnectionInfo,Result,SchemaTable } from "./types";
import { message,translateCopy,type LocalizedCopy } from "../../shared/i18n";
export function useDataEvents(sessionId:string|undefined,events:ProtocolEvent[],dropped:number) {
  const [info,setInfo]=useState<ConnectionInfo|null>(null),[tables,setTables]=useState<SchemaTable[]>([]),[schemaTruncated,setSchemaTruncated]=useState(false);
  const [result,setResult]=useState<Result|null>(null),[active,setActive]=useState<string|null>(null),[error,setError]=useState<string|LocalizedCopy>("");
  const cursor=useRef(0),scope=useRef(sessionId),query=useRef<string|null>(null),schemaGap=useRef(false),inFlight=useRef(false);
  useEffect(()=>{
    if(scope.current!==sessionId){scope.current=sessionId;cursor.current=0;query.current=null;inFlight.current=false;schemaGap.current=false;setInfo(null);setTables([]);setResult(null);setActive(null);setError("");setSchemaTruncated(false);}
    const first=events.find(event=>event.cursor>cursor.current);
    if(first&&first.cursor>cursor.current+1){query.current=null;inFlight.current=false;schemaGap.current=true;setTables([]);setResult(null);setActive(null);setSchemaTruncated(true);setError(message("结果事件已被淘汰，请重新运行查询。"));}
    for(const event of events){if(event.cursor<=cursor.current)continue;cursor.current=event.cursor;const m=event.message;
      switch(m.kind){
        case "data_ready":setInfo(m.info);break;
        case "data_schema":if(m.clear)schemaGap.current=false;setTables(old=>m.clear?m.tables:[...old,...m.tables]);setSchemaTruncated(schemaGap.current||m.truncated);break;
        case "state":if(m.state==="closed"||m.state==="error"){if(inFlight.current){setResult(null);setError(message("连接在查询完成前关闭，请重新连接。"));}inFlight.current=false;query.current=null;setActive(null);}break;
        case "data_started":if(query.current&&query.current!==m.query_id)break;query.current=m.query_id;inFlight.current=true;setActive(m.query_id);setError("");setResult({id:m.query_id,columns:[],rows:[],elapsed_ms:0,rows_affected:0,truncated:false,limit_reason:null,done:false});break;
        case "data_columns":setResult(old=>old?.id===m.query_id?{...old,columns:m.columns}:old);break;
        case "data_rows":setResult(old=>old?.id===m.query_id?{...old,rows:[...old.rows,...m.rows]}:old);break;
        case "data_finished":if(query.current===m.query_id)inFlight.current=false;setActive(old=>old===m.query_id?null:old);setResult(old=>old?.id===m.query_id?{...old,elapsed_ms:m.elapsed_ms,rows_affected:m.rows_affected,truncated:m.truncated,limit_reason:m.limit_reason,done:true}:old);break;
        case "data_error":if(m.query_id&&query.current!==m.query_id)break;if(m.query_id)inFlight.current=false;setActive(old=>old===m.query_id?null:old);setError(m.message);setResult(old=>old?.id===m.query_id?{...old,done:true}:old);break;
        case "data_cancelled":if(query.current!==m.query_id)break;inFlight.current=false;setActive(old=>old===m.query_id?null:old);setError(m.write_outcome_unknown?message("已请求取消并关闭连接，写入结果可能未知。"):message("已请求取消并关闭连接。"));setResult(null);break;
      }
    }
  },[sessionId,events,dropped]);
  return {info,tables,schemaTruncated,result,active,error:translateCopy(error),reserve:(id:string)=>{if(inFlight.current)return false;query.current=id;inFlight.current=true;setActive(id);setError("");return true;},release:(id:string)=>{if(query.current===id){query.current=null;inFlight.current=false;setActive(null);}}};
}
