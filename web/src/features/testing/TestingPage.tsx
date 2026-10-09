import {t,useLanguage} from "../../shared/i18n";
import {useState} from "react";
import {Badge,Button,Card,Flex,Heading,Text,Callout} from "@radix-ui/themes";
import {FlaskConical} from "lucide-react";
import {Choice} from "../../shared/ui";
import {useWorkbench} from "../workbench/context";
import RunnerDataset,{runnerOptions,type RunnerConfig} from "./RunnerDataset";
import SavedDatasets from "./SavedDatasets";
export default function TestingPage(){
 useLanguage();const state=useWorkbench();const {draft,runCollection,setRunCollection,runResult,runnerBusy,run,stopRunner}=state;
 const scope=JSON.stringify([state.accountId,draft?.id]);
 const [source,setSource]=useState<{scope:string;value:RunnerConfig}|null>(null),[invalid,setInvalid]=useState(false);
 const [previewBusy,setPreviewBusy]=useState<{scope:string;busy:boolean}|null>(null);
 const dataBusy=previewBusy?.scope===scope&&previewBusy.busy;
 const config=source?.scope===scope?source.value:{format:"json" as const,source:"",iterations:""};
 const [savedSelection,setSavedSelection]=useState<{scope:string;id:string}|null>(null);
 const savedId=savedSelection?.scope===scope&&draft?.data.datasets?.some(dataset=>dataset.id===savedSelection.id)?savedSelection.id:"";
 const saved=draft?.data.datasets?.find(dataset=>dataset.id===savedId);
 function start(){try{const options=runnerOptions(savedId?{...config,source:""}:config);if(savedId){if(!saved?.source)throw new Error("dataset source missing");options.dataset_id=savedId;}setInvalid(false);void run(options);}catch{setInvalid(true);}}
 if(!draft)return null;
 return <div className="page-panel"><div className="page-heading"><div><Heading size="5">{t("集合测试")}</Heading><Text size="2" color="gray">{t("按顺序运行集合内的请求，并检查断言。")}</Text></div></div>
 <Flex gap="3" align="center" wrap="wrap"><Choice value={runCollection||draft.data.collections[0]?.id||"none"} onChange={setRunCollection} options={draft.data.collections.map(collection=>({value:collection.id,label:collection.name}))} label={t("测试集合")} disabled={runnerBusy}/><Button loading={runnerBusy} disabled={!draft.data.collections.length||dataBusy} onClick={start}><FlaskConical size={16}/>{t("运行集合")}</Button>{runnerBusy&&<Button variant="soft" onClick={()=>void stopRunner()}>{t("停止集合运行")}</Button>}</Flex>
 <Choice label={t("运行数据集")} value={savedId||"temporary"} disabled={runnerBusy} options={[{value:"temporary",label:t("临时数据 / 无数据")},...(draft.data.datasets??[]).map(dataset=>({value:dataset.id,label:dataset.name}))]} onChange={id=>{setSavedSelection({scope,id:id==="temporary"?"":id});setInvalid(false);}}/>
 <RunnerDataset key={scope+savedId} workspaceId={draft.id} scope={scope} value={savedId?{...config,format:saved?.source?.format??"json",source:saved?.source?.source??""}:config} onBusyChange={busy=>setPreviewBusy({scope,busy})} disabled={runnerBusy} sourceReadOnly={!!savedId} dark={state.dark} onChange={value=>{setSource({scope,value:savedId?{...config,iterations:value.iterations}:value});setInvalid(false);}}/>
 {savedId&&!saved?.source&&<Text>{t("数据源缺失，请重新导入。")}</Text>}
 <SavedDatasets key={scope+draft.revision} scope={scope+draft.revision} disabled={runnerBusy}/>
 {invalid&&<Callout.Root color="red"><Callout.Text role="alert">{t("测试数据无效，请检查格式、迭代次数和大小限制。")}</Callout.Text></Callout.Root>}
 {runResult&&<><Flex gap="4" align="center"><Badge color="green">{t("通过")} {runResult.passed}</Badge><Badge color="red">{t("失败")} {runResult.failed}</Badge><Text size="2" color="gray">{runResult.elapsed_ms} ms</Text></Flex>
 {runResult.stopped_reason&&<Text role="status">{runResult.stopped_reason==="deadline"?t("集合运行达到时间限制。"):runResult.stopped_reason==="variable_limit"?t("集合运行达到变量大小限制。"):t("集合运行已取消。")}</Text>}
 {runResult.iterations?.map(iteration=><Text key={iteration.iteration}>{t("第 {{iteration}} 轮：通过 {{passed}}，失败 {{failed}}。",{iteration:iteration.iteration+1,passed:iteration.passed,failed:iteration.failed})}</Text>)}
 {runResult.results.map((item,index)=><Card key={`${item.iteration??0}:${item.collection_id??""}:${item.request_id}:${index}`}><Flex justify="between"><Text weight="medium">{item.request_name}</Text>{item.iteration!==undefined&&<Text size="1">{t("第 {{iteration}} 轮",{iteration:item.iteration+1})}</Text>}{(item.response||item.status)&&<Badge color={item.response?item.response.tests.every(test=>test.passed)?"green":"red":"gray"}>{item.response?.status??item.status}</Badge>}</Flex>{item.error&&<Text color="red" size="2">{item.error}</Text>}{item.response_omitted&&<Text size="1">{t("响应详情因报告大小限制被省略。")}</Text>}{item.response?.tests.map(test=><Text as="p" size="2" key={test.id} color={test.passed?"green":"red"}>{test.passed?t("通过"):t("失败")} · {test.name} · {test.actual}</Text>)}</Card>)}</>}
 </div>;
}
