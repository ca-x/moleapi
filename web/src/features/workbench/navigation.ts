import { t } from "../../shared/i18n";
import { BookOpen, Cloud, FlaskConical, History, Webhook, Workflow } from "lucide-react";
export type View =
  "requests" | "environments" | "history" | "runner" | "specifications" | "webhooks";
export const nav = [
  { id: "webhooks", get label() { return t("回调"); }, icon: Webhook },
  { id: "requests", get label() { return t("接口"); }, icon: Workflow },
  { id: "environments", get label() { return t("环境"); }, icon: Cloud },
  { id: "history", get label() { return t("历史"); }, icon: History },
  { id: "runner", get label() { return t("测试"); }, icon: FlaskConical },
  { id: "specifications", get label() { return t("规范"); }, icon: BookOpen },
] as const;
