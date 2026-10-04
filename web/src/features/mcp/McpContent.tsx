import { Badge, Flex, Text } from "@radix-ui/themes";
import { Editor } from "../../shared/ui";
import { object, resultContents } from "./model";
const images = new Set(["image/png", "image/jpeg", "image/gif", "image/webp"]);
const audio = new Set(["audio/mpeg", "audio/mp3", "audio/ogg", "audio/wav", "audio/webm", "audio/flac"]);
export default function McpContent({ result, dark }: { result: unknown; dark: boolean }) {
  const blocks = resultContents(result);
  const value = object(result);
  return <div className="mcp-result-content">
    {value?.isError === true && <Badge color="red">工具返回错误</Badge>}
    {blocks.length > 64 && <Text size="1" color="gray">仅预览前 64 条内容；完整内容见响应 JSON。</Text>}
    {blocks.slice(0,64).map((block, index) => {
      const type = block.type;
      const data = typeof block.data === "string" && block.data.length <= 1_500_000 && /^[A-Za-z0-9+/]*={0,2}$/.test(block.data) ? block.data : null;
      const mime = typeof block.mimeType === "string" ? block.mimeType : "";
      if (type === "image" && data && images.has(mime))
        return <img key={index} className="mcp-image" src={"data:"+mime+";base64,"+data} alt={"MCP 返回图像 " + (index + 1)} />;
      if (type === "audio" && data && audio.has(mime))
        return <audio key={index} controls preload="none" src={"data:"+mime+";base64,"+data} aria-label={"MCP 返回音频 " + (index + 1)} />;
      const resource = type === "resource" ? object(block.resource) : null;
      if (type === "text" || (resource && typeof resource.text === "string")) {
        const text = resource ? resource.text as string : typeof block.text === "string" ? block.text : "";
        return <div key={index}>{resource && <Text size="1" color="gray">{String(resource.uri || "资源")}</Text>}
          <Editor value={text} dark={dark} readOnly height="180px" label={"MCP 内容 " + (index+1)} />
        </div>;
      }
      if (type === "resource_link") return <Flex direction="column" key={index} gap="1">
        <Text weight="medium">{String(block.name || "资源链接")}</Text>
        <Text size="1" className="mono">{String(block.uri || "")}</Text>
        <Text size="1" color="gray">可在资源请求中读取此 URI。</Text>
      </Flex>;
      return <Editor key={index} value={JSON.stringify(block, null, 2)} dark={dark} jsonMode readOnly height="180px" label={"MCP 内容 " + (index+1)} />;
    })}
    <Editor value={JSON.stringify(result, null, 2) || "null"} dark={dark} jsonMode readOnly height="240px" label="MCP 响应 JSON" />
  </div>;
}
