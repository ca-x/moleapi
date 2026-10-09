import {lazy,Suspense,useState} from "react";
const ProjectGenerationDialog=lazy(()=>import("../generation/ProjectGenerationDialog"));
import { t, useLanguage } from "../../shared/i18n";
import { Badge, Button, Card, Flex, Heading, Text } from "@radix-ui/themes";
import { Upload } from "lucide-react";
import { Editor } from "../../shared/ui";
import { useWorkbench } from "../workbench/context";

export default function SpecificationsPage() {
  useLanguage();
  const state = useWorkbench();
  const [projectSpec,setProjectSpec]=useState<string|null>(null);
  const { dark, draft, openModal } = state;
  if (!draft) return null;
  return (
    <div className="page-panel">
      <div className="page-heading">
        <div>
          <Heading size="5">{t("API 规范")}</Heading>
          <Text size="2" color="gray"> {t("保留原始规范，与导入的可执行请求关联。")} </Text>
        </div>
        <Button onClick={() => openModal("import")}>
          <Upload size={16} /> {t("导入规范")} </Button>
      </div>
      {draft.data.specifications?.length ? (
        draft.data.specifications.map((spec) => (
          <Card key={spec.id}>
            <Flex justify="between" align="center">
              <Heading size="3">{spec.name}</Heading>
              {["openapi","protobuf"].includes(spec.kind)&&<Button variant="soft" onClick={()=>setProjectSpec(spec.id)}>{t("生成 SDK / 服务端项目")}</Button>}
              <Badge color="gray">
                {spec.kind} {spec.dialect}
              </Badge>
            </Flex>
            <Editor value={spec.source} dark={dark} readOnly height="360px" />
          </Card>
        ))
      ) : (
        <Text color="gray"> {t("导入 OpenAPI JSON/YAML 后，原始规范将在此保留。")} </Text>
      )}
      {projectSpec&&<Suspense fallback={<Text role="status">{t("正在读取生成器…")}</Text>}><ProjectGenerationDialog open specificationId={projectSpec} onOpenChange={open=>{if(!open)setProjectSpec(null);}}/></Suspense>}
    </div>
  );
}
