import {expect,it} from "vitest";
import {parseTemplateBundle,templateKinds} from "./templateBundle";
const files=[{path:"extra.mustache",content:"{{appName}}"}];
it("restores every upstream output role and preserves explicitly mapped static assets",()=>{
 for(const templateType of templateKinds){
  const value={format:"moleapi-codegen-templates-v1",files,outputs:{"extra.mustache":{templateType,destinationFilename:"Custom.txt",...(templateType==="SupportingFiles"?{folder:"extras"}:{})}}};
  expect(parseTemplateBundle(value)).toEqual(value);
 }
 const value={format:"moleapi-codegen-templates-v1",files:[{path:"AUTHORS.md",content:"{{ not rendered }}"}],outputs:{"AUTHORS.md":{}}};expect(parseTemplateBundle(value)).toEqual(value);
 const binary={format:"moleapi-codegen-templates-v1",files:[{path:"asset.bin",encoding:"base64",content:"AP+A"}],outputs:{"asset.bin":{}}};expect(parseTemplateBundle(binary)).toEqual(binary);
 expect(()=>parseTemplateBundle({...binary,files:[{path:"asset.mustache",encoding:"base64",content:"AP+A"}],outputs:{"asset.mustache":{destinationFilename:".txt"}}})).toThrow();
 expect(()=>parseTemplateBundle({...binary,files:[{path:"asset.bin",encoding:"base64",content:"not base64"}]})).toThrow();
});
it("rejects missing sources, path escapes, reserved metadata and colliding supporting outputs",()=>{
 const base={format:"moleapi-codegen-templates-v1",files};
 for(const outputs of [{"missing.mustache":{destinationFilename:"Extra.md"}},{"extra.mustache":{destinationFilename:"../outside"}},{"extra.mustache":{folder:"moleapi-generation.json",destinationFilename:"Extra.md"}},{"extra.mustache":{templateType:"Model",folder:"extras",destinationFilename:".md"}}])expect(()=>parseTemplateBundle({...base,outputs})).toThrow();
 expect(()=>parseTemplateBundle({format:"moleapi-codegen-templates-v1",files:[...files,{path:"other.mustache",content:""}],outputs:{"extra.mustache":{destinationFilename:"same"},"other.mustache":{folder:"same",destinationFilename:"file.md"}}})).toThrow();
});
