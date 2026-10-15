import type { DubboConfig } from "../../shared/types";

export function dubboConfig(): DubboConfig {
  return {
    kind: "dubbo", source_version: 1, source: "direct", transport: "classic", serialization: "hessian2",
    service: "", method: "", version: "", group: "", arguments: [], attachments: [],
  };
}
