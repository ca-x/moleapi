import {t} from "../../shared/i18n";
import {ResultDatasetError} from "./dataset";
export function resultDatasetMessage(error:unknown){
  if(error instanceof ResultDatasetError)switch(error.code){
    case "incomplete":return t("查询尚未完成，不能保存数据集。");
    case "failed":return t("查询失败，不能保存不完整的数据集。");
    case "truncated":return t("查询结果已截断，请缩小 SQL 结果后重试。");
    case "empty":return t("查询没有返回行，不能保存数据集。");
    case "rows":return t("数据集最多保存 100 行，请在 SQL 中缩小结果。");
    case "columns":return t("数据集最多保存 100 列，且每行列数必须一致。");
    case "duplicate_columns":return t("列名必须非空且唯一，请在 SQL 中为重复列起别名。");
    case "unsupported_cell":return t("二进制或截断单元格不能保存为数据集，请在 SQL 中转换后重试。");
    case "name":return t("请输入 1–256 字节的数据集名称。");
    case "datasets":return t("工作区最多保存 20 个数据集。");
    case "aggregate":return t("工作区数据集源总量超过 4 MiB。");
    case "source_size":return t("数据集 JSON 超过 1 MiB，请缩小 SQL 结果。");
  }
  return t("数据集保存失败，请检查数据或工作区版本后重试。");
}

