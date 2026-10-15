import {useEffect, useRef, useState} from "react";
import {Button, Flex, Text} from "@radix-ui/themes";
import {FolderOpen} from "lucide-react";
import {api} from "../../shared/api";
import {t, useLanguage} from "../../shared/i18n";
import type {ProjectCatalog} from "./projectTypes";
import {selectNativeJavaRuntime} from "./selectNativeJavaRuntime";

export default function NativeJavaRuntime({available, disabled, onConfigured}: {
  available: boolean;
  disabled: boolean;
  onConfigured: (catalog: ProjectCatalog) => void;
}) {
  useLanguage();
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState(false);
  const alive = useRef(true);
  const selecting = useRef(false);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  async function select() {
    if (selecting.current || disabled) return;
    selecting.current = true;
    setPending(true);
    setFailed(false);
    try {
      const major = await selectNativeJavaRuntime();
      if (major === null || !alive.current) return;
      const catalog = await api<ProjectCatalog>("/api/generation/projects/catalog");
      if (alive.current) onConfigured(catalog);
    } catch {
      if (alive.current) setFailed(true);
    } finally {
      selecting.current = false;
      if (alive.current) setPending(false);
    }
  }
  return <Flex direction="column" gap="2" aria-busy={pending}>
    <Flex align="center" justify="between" gap="3" wrap="wrap">
      <Flex direction="column" gap="1" style={{minWidth: 0}}>
        <Text size="2" weight="medium">Java 17+</Text>
        <Text size="2" color="gray" role="status">
          {pending ? t("正在选择并检查 Java…") : available
            ? t("Java 已配置，将用于新建的生成任务。")
            : t("选择已安装的 Java 17+ 可执行文件，配置会保存在此设备。")}
        </Text>
      </Flex>
      <Button variant="soft" highContrast disabled={disabled || pending} onClick={() => void select()} style={{minHeight: 44, flexShrink: 0}}>
        <FolderOpen size={16} aria-hidden="true"/>{available ? t("更换 Java…") : t("选择 Java…")}
      </Button>
    </Flex>
    {failed && <Text size="2" color="red" role="alert">{t("无法配置 Java，请选择 Java 17+ 可执行文件，并检查应用设置的写入权限。")}</Text>}
  </Flex>;
}
