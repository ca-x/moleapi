import {z} from "zod";
import {base64} from "@scure/base";
import {projectRelativePath} from "./projectInputs";
export const templateKinds=["API","APIDocs","APITests","Model","ModelDocs","ModelTests","SupportingFiles"] as const;
const output=z.object({templateType:z.enum(templateKinds).optional(),folder:z.string().max(256).optional(),destinationFilename:z.string().min(1).max(256).optional()}).strict();
const schema=z.object({format:z.literal("moleapi-codegen-templates-v1"),files:z.array(z.object({path:z.string().max(256),content:z.string().max(87384),encoding:z.enum(["utf8","base64"]).optional()}).strict()).min(1).max(128),outputs:z.record(z.string().max(256),output).optional()}).strict();
export type TemplateBundle=z.infer<typeof schema>;
export type TemplateOutput=z.infer<typeof output>;
const safePath=(path:string)=>{projectRelativePath(path);if(path.length>256||!/^[a-zA-Z0-9_./-]+$/.test(path))throw new Error("template path");};
export function parseTemplateBundle(value:unknown):TemplateBundle {
 const bundle=schema.parse(value);
 if(new TextEncoder().encode(JSON.stringify(bundle)).length>512*1024)throw new Error("template limit");
 const paths=new Set<string>();
 for(const file of bundle.files){safePath(file.path);const bytes=file.encoding==="base64"?base64.decode(file.content):new TextEncoder().encode(file.content);if(paths.has(file.path)||bytes.length>64*1024)throw new Error("invalid template");paths.add(file.path);
  if(file.path.endsWith(".mustache")&&(file.encoding==="base64"||file.content.includes("\0")))throw new Error("mustache text");
  if(!file.path.endsWith(".mustache")&&(!Object.hasOwn(bundle.outputs??{},file.path)||(bundle.outputs![file.path].templateType??"SupportingFiles")!=="SupportingFiles"))throw new Error("static output mapping");
 }
 const entries=Object.entries(bundle.outputs??{});if(entries.length>128)throw new Error("output limit");
 const destinations=new Set<string>(),supporting=new Set<string>();
 for(const [source,definition] of entries){
  if(!paths.has(source))throw new Error("missing source");const kind=definition.templateType??"SupportingFiles";
  if(definition.folder){safePath(definition.folder);if(kind!=="SupportingFiles")throw new Error("output folder");}
  const filename=definition.destinationFilename??source;safePath(filename);
  if(definition.destinationFilename&&filename.includes("/"))throw new Error("output filename");
  if(source.endsWith(".mustache")&&!definition.destinationFilename)throw new Error("missing output filename");
  const path=definition.folder?`${definition.folder}/${filename}`:filename;
  if(["moleapi-generation.json","moleapi-templates.json","moleapi-regeneration.json"].some(reserved=>path===reserved||path.startsWith(reserved+"/")))throw new Error("reserved output");
  const key=kind+":"+path;if(destinations.has(key))throw new Error("duplicate output");destinations.add(key);if(kind==="SupportingFiles")supporting.add(path);
 }
 for(const path of supporting){const segments=path.split("/");for(let i=1;i<segments.length;i++)if(supporting.has(segments.slice(0,i).join("/")))throw new Error("output collision");}
 return bundle;
}
