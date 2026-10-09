import { liveError } from "./../../shared/i18n/errors";
import { useLanguage } from "../../shared/i18n";
import { useEffect,useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api } from "../../shared/api";
import type { RunResult,RunOptions } from "../../shared/types";
import {id} from "../../shared/model";
import type { useLocalVariables } from "../variables/useLocalVariables";
import type { useWorkspace } from "../workspaces/useWorkspace";

export function useRunner(
  workspace: ReturnType<typeof useWorkspace>,
  localVariables?: ReturnType<typeof useLocalVariables>,
) {
  useLanguage();
  const { draft, dirty, save, stateRef, accountId, accountRef } = workspace;
  const client = useQueryClient();
  const [selected, setRunCollection] = useState("");
  const runCollection = draft?.data.collections.some((c) => c.id === selected)
    ? selected
    : draft?.data.collections[0]?.id || "";
  const [result, setResult] = useState<{
    accountId: string;
    workspaceId: string;
    collectionId: string;
    value: RunResult;
  } | null>(null);
  const [runnerBusy, setRunnerBusy] = useState(false);
  const running = useRef(false);
  const job=useRef<{id:string;workspaceId:string}|null>(null);
  const cancelled=useRef(new Set<string>());
  async function stopRunner(){const previous=job.current;if(previous){cancelled.current.add(previous.id);try{await api(`/api/workspaces/${previous.workspaceId}/run/cancel`,"POST",{job_id:previous.id});}catch(error){toast.error(liveError(error));}}}
  useEffect(()=>()=>{const previous=job.current;job.current=null;if(previous)void api(`/api/workspaces/${previous.workspaceId}/run/cancel`,"POST",{job_id:previous.id}).catch(()=>{});},[accountId,draft?.id]);
  async function run(options:RunOptions={}) {
    if (!draft || !runCollection || running.current) return;
    const workspaceId = draft.id;
    const collectionId = runCollection;
    const environmentId = draft.data.active_environment_id;
    running.current = true;
    const ticket=id();job.current={id:ticket,workspaceId};
    setRunnerBusy(true);
    setResult(null);
    try {
      if (dirty && !(await save(true))) return;
      if (
        accountRef.current !== accountId ||
        stateRef.current.draft?.id !== workspaceId
      )
        return;
      if(cancelled.current.has(ticket)){setResult({accountId,workspaceId,collectionId,value:{results:[],passed:0,failed:0,elapsed_ms:0,cancelled:true,stopped_reason:"cancelled"}});return;}
      const locals =
        localVariables?.values(draft, collectionId, environmentId) || [];
      const value = await api<RunResult>(
        `/api/workspaces/${workspaceId}/run`,
        "POST",
        {
          collection_id: collectionId,
          environment_id: environmentId,
          ...(locals.length ? { locals } : {}),
          ...options,job_id:ticket,
        },
      );
      if (
        accountRef.current === accountId &&
        stateRef.current.draft?.id === workspaceId
      ) {
        for (const result of value.results)
          localVariables?.apply(
            draft,
            result.collection_id??collectionId,
            environmentId,
            result.response?.variable_updates || [],
          );
      }
      setResult({ accountId, workspaceId, collectionId, value });
      void client.invalidateQueries({ queryKey: ["history", workspaceId] });
    } catch (error) {
      if(accountRef.current===accountId&&stateRef.current.draft?.id===workspaceId)toast.error(liveError(error));
    } finally {
      if(job.current?.id===ticket)job.current=null;
      cancelled.current.delete(ticket);
      running.current = false;
      setRunnerBusy(false);
    }
  }
  return {
    runCollection,
    setRunCollection,
    runnerBusy,
    runResult:
      result?.accountId === accountId &&
      result?.workspaceId === draft?.id &&
      result?.collectionId === runCollection
        ? result.value
        : null,
    run,
    stopRunner,
  };
}
