import { BookOpen, Cloud, FlaskConical, History, Workflow } from "lucide-react";
export type View =
  "requests" | "environments" | "history" | "runner" | "specifications";
export const nav = [
  { id: "requests", label: "接口", icon: Workflow },
  { id: "environments", label: "环境", icon: Cloud },
  { id: "history", label: "历史", icon: History },
  { id: "runner", label: "测试", icon: FlaskConical },
  { id: "specifications", label: "规范", icon: BookOpen },
] as const;
