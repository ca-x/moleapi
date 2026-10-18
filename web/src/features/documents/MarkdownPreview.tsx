import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {t} from "../../shared/i18n";

type MarkdownNode={type:string;depth?:number;children?:MarkdownNode[]};
function nestedHeadings() {
  return (tree:MarkdownNode)=>{
    let firstDepth:number|undefined;
    const visit=(node:MarkdownNode)=>{
      if(node.type==="heading"&&node.depth!==undefined) {
        firstDepth??=node.depth;
        node.depth=Math.min(6,Math.max(4,4+node.depth-firstDepth));
      }
      node.children?.forEach(visit);
    };
    visit(tree);
  };
}

function safeLink(raw?:string):string {
  if(!raw)return "";
  try {
    const url=new URL(raw);
    return ["https:","http:","mailto:"].includes(url.protocol)?url.href:"";
  } catch {return "";}
}

export default function MarkdownPreview({source,nested=false}:{source:string;nested?:boolean}) {
  return <div className="markdown-preview">
    <ReactMarkdown skipHtml remarkPlugins={nested?[remarkGfm,nestedHeadings]:[remarkGfm]} components={{
      a:({href,children})=>{
        const safe=safeLink(href);
        return safe?<a href={safe} target="_blank" rel="noopener noreferrer">{children}</a>:<span>{children}</span>;
      },
      img:({alt})=><span>{alt||t("图片已省略")}</span>
    }}>{source}</ReactMarkdown>
  </div>;
}
