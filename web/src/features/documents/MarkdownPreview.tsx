import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {t} from "../../shared/i18n";

function safeLink(raw?:string):string {
  if(!raw)return "";
  try {
    const url=new URL(raw);
    return ["https:","http:","mailto:"].includes(url.protocol)?url.href:"";
  } catch {return "";}
}

export default function MarkdownPreview({source,nested=false}:{source:string;nested?:boolean}) {
  return <div className="markdown-preview">
    <ReactMarkdown skipHtml remarkPlugins={[remarkGfm]} components={{
      a:({href,children})=>{
        const safe=safeLink(href);
        return safe?<a href={safe} target="_blank" rel="noopener noreferrer">{children}</a>:<span>{children}</span>;
      },
      img:({alt})=><span>{alt||t("图片已省略")}</span>,
      h1:({children})=>nested?<h4>{children}</h4>:<h1>{children}</h1>,
      h2:({children})=>nested?<h4>{children}</h4>:<h2>{children}</h2>,
      h3:({children})=>nested?<h4>{children}</h4>:<h3>{children}</h3>,
      h4:({children})=>nested?<h4>{children}</h4>:<h4>{children}</h4>,
      h5:({children})=>nested?<h4>{children}</h4>:<h5>{children}</h5>,
      h6:({children})=>nested?<h4>{children}</h4>:<h6>{children}</h6>
    }}>{source}</ReactMarkdown>
  </div>;
}
