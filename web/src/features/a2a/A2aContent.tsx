import { Badge, Flex, Text } from "@radix-ui/themes";
import { Editor } from "../../shared/ui";
import { object } from "../mcp/model";
import McpContent from "../mcp/McpContent";
/** Display SDK-decoded parts without following URLs or executing agent HTML. */
export default function A2aContent({result,dark}:{result:unknown;dark:boolean}) {
  const value=object(result);
  const task=object(value?.task) || object(value?.statusUpdate) || (value?.status?value:null);
  const status=object(task?.status);
  const message=object(value?.message) || (Array.isArray(value?.parts)?value:null) || object(status?.message);
  const artifact=object(value?.artifact)||object(object(value?.artifactUpdate)?.artifact);
  const parts:unknown[]=[];
  if(Array.isArray(message?.parts))parts.push(...message.parts);
  for(const item of Array.isArray(task?.history)?task.history:[]) { const message=object(item);if(Array.isArray(message?.parts))parts.push(...message.parts); }
  for(const raw of [...(Array.isArray(task?.artifacts)?task.artifacts:[]),...(artifact?[artifact]:[])]) {
    const entry=object(raw);if(Array.isArray(entry?.parts))parts.push(...entry.parts);
  }
  const content=parts.map(raw=>{
    const part=object(raw);if(!part)return {type:"text",text:String(raw)};
    if(typeof part.text==="string")return {type:"text",text:part.text};
    const file=object(part.file);
    if(file)return {type:"resource",resource:{uri:String(file.uri||file.url||file.name||"文件"),mimeType:file.mimeType||part.mediaType,text:JSON.stringify(file,null,2)}};
    return {type:"text",text:JSON.stringify(part,null,2)};
  });
  return <div className="a2a-content">
    {task&&<Flex gap="3" wrap="wrap">{status&&<Badge color="gray">{String(status.state||"任务")}</Badge>}<Text size="1" className="mono">任务 {String(task.id||task.taskId||"")}</Text>{typeof task.contextId === "string" && task.contextId && <Text size="1" className="mono">上下文 {String(task.contextId)}</Text>}</Flex>}
    {content.length>0&&<McpContent result={{content}} dark={dark}/>}
    <Editor value={JSON.stringify(result,null,2)||"null"} jsonMode dark={dark} readOnly height="280px" label="A2A 响应 JSON"/>
  </div>;
}
