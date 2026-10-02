import { Badge, Button, Card, Flex, Heading, Text } from "@radix-ui/themes";
import { Upload } from "lucide-react";
import { Editor } from "../../shared/ui";
import { useWorkbench } from "../workbench/context";

export default function SpecificationsPage() {
  const state = useWorkbench();
  const { dark, draft, openModal } = state;
  if (!draft) return null;
  return (
    <div className="page-panel">
      <div className="page-heading">
        <div>
          <Heading size="5">API 规范</Heading>
          <Text size="2" color="gray">
            保留原始规范，与导入的可执行请求关联。
          </Text>
        </div>
        <Button onClick={() => openModal("import")}>
          <Upload size={16} />
          导入规范
        </Button>
      </div>
      {draft.data.specifications?.length ? (
        draft.data.specifications.map((spec) => (
          <Card key={spec.id}>
            <Flex justify="between" align="center">
              <Heading size="3">{spec.name}</Heading>
              <Badge color="gray">
                {spec.kind} {spec.dialect}
              </Badge>
            </Flex>
            <Editor value={spec.source} dark={dark} readOnly height="360px" />
          </Card>
        ))
      ) : (
        <Text color="gray">
          导入 OpenAPI JSON/YAML 后，原始规范将在此保留。
        </Text>
      )}
    </div>
  );
}
