import { useEffect,useMemo,useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "./api";
import type { BootstrapData,MasterData } from "./types";
import { commonContacts,commonDepartments } from "./lib/task-utils";
import TaskForm from "./components/TaskForm";

const emptyMasters:MasterData={departments:[],taskTypes:[],contacts:[]};

export default function QuickAddWindow(){
  const [data,setData]=useState<BootstrapData|null>(null);
  const [error,setError]=useState("");
  const load=async()=>{setError("");try{setData(await api.bootstrap());}catch(reason){setError(reason instanceof Error?reason.message:String(reason));}};
  useEffect(()=>{void load();return api.onDataChanged(()=>void load());},[]);
  const history=useMemo(()=>data?[...data.queue,...data.archive].sort((a,b)=>a.updatedAt.localeCompare(b.updatedAt)):[],[data]);
  const departments=useMemo(()=>commonDepartments(history),[history]);
  const contacts=useMemo(()=>commonContacts(history),[history]);
  const close=()=>void getCurrentWindow().hide();
  if(!data)return <main className="quick-add-loading"><img src="/inline-mark.svg"/><strong>{error?"暂时无法载入新增页面":"正在准备新增取号…"}</strong>{error&&<><p>{error}</p><button className="button primary" onClick={()=>void load()}>重试</button></>}</main>;
  return <div className="quick-add-window"><TaskForm task={null} masters={data.masters??emptyMasters} commonDepartments={departments} commonContacts={contacts} onClose={close} onSaved={()=>{void load();close();}}/></div>;
}
