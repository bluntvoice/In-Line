import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import { Image } from "@tauri-apps/api/image";
import { writeImage,writeText } from "@tauri-apps/plugin-clipboard-manager";
import type { BackupInfo,BackupMergeResult,BootstrapData,LegalTask,MasterData,MergeTaskInput,MoveDirection,QueueInput,StatisticsDetail,StatisticsResult,TaskInput,TaskLog,TaskStatus,TaskUiAction,TaskView,TaskWorkEvent,TicketSnapshot } from "./types";
import { renderTicketPng,renderTicketRgba,warmTicketRenderer } from "./lib/ticket-image";

let pngImageSupported=true;

if(typeof window!=="undefined"){
  const warmup=()=>{void warmTicketRenderer().catch(()=>undefined);};
  if("requestIdleCallback" in window)window.requestIdleCallback(warmup,{timeout:1500});
  else globalThis.setTimeout(warmup,0);
}

const withTimeout=<T>(request:Promise<T>,label:string,timeoutMs=12000)=>new Promise<T>((resolve,reject)=>{
  const timer=window.setTimeout(()=>reject(new Error(`${label}超时，请重新载入；如仍失败，请确认旧版程序已退出。`)),timeoutMs);
  request.then(value=>{
    window.clearTimeout(timer);
    resolve(value);
  },error=>{
    window.clearTimeout(timer);
    reject(error);
  });
});

export const api={
  bootstrap:()=>withTimeout(invoke<BootstrapData>("bootstrap"),"队列初始化"),
  listTasks:(view:TaskView)=>withTimeout(invoke<LegalTask[]>("list_tasks",{view}),"队列载入"),
  saveTask:(task:TaskInput)=>invoke<LegalTask>("save_task",{task}),
  setTaskStatus:(id:number,status:TaskStatus)=>invoke<void>("set_task_status",{id,status}),
  moveTask:(id:number,direction:MoveDirection)=>invoke<void>("move_task",{id,direction}),
  deleteTask:(id:number)=>invoke<void>("delete_task",{id}),
  restoreTask:(id:number)=>invoke<void>("restore_task",{id}),
  permanentlyDeleteTasks:(ids:number[])=>invoke<number>("permanently_delete_tasks",{ids}),
  emptyTrash:()=>invoke<number>("empty_trash"),
  archiveTask:(id:number)=>invoke<void>("archive_task",{id}),
  mergeTasks:(input:MergeTaskInput)=>invoke<void>("merge_tasks",{input}),
  resolveImportConflict:(id:number)=>invoke<void>("resolve_import_conflict",{id}),
  getLogs:(taskId:number)=>invoke<TaskLog[]>("get_logs",{taskId}),
  getWorkEvents:(taskId:number)=>invoke<TaskWorkEvent[]>("get_work_events",{taskId}),
  voidWorkEvent:(id:number,confirmHistoricalImpact=false)=>invoke<void>("void_work_event",{id,confirmHistoricalImpact}),
  processRound:(id:number)=>invoke<void>("process_round",{id}),
  completeRound:(id:number)=>invoke<void>("complete_round",{id}),
  enqueueTask:(input:QueueInput)=>invoke<void>("enqueue_task",{input}),
  reopenTask:(input:QueueInput)=>invoke<void>("reopen_task",{input}),
  getStatistics:(start:string,end:string)=>invoke<StatisticsResult>("get_statistics",{start,end,timezoneOffsetMinutes:-new Date().getTimezoneOffset()}),
  getStatisticsDetails:(start:string,end:string,taskType:string)=>invoke<StatisticsDetail[]>("get_statistics_details",{start,end,taskType}),
  addLog:(taskId:number,content:string)=>invoke<void>("add_log",{taskId,content}),
  updateLog:(logId:number,content:string)=>invoke<void>("update_log",{logId,content}),
  deleteLog:(logId:number)=>invoke<void>("delete_log",{logId}),
  addMaster:(kind:"department"|"task_type"|"contact",name:string)=>invoke<MasterData>("add_master",{kind,name}),
  deleteMaster:(kind:"department"|"task_type"|"contact",name:string)=>invoke<MasterData>("delete_master",{kind,name}),
  moveMaster:(kind:"department"|"task_type",name:string,direction:MoveDirection)=>invoke<MasterData>("move_master",{kind,name,direction}),
  listBackups:()=>invoke<BackupInfo[]>("list_backups"),
  createBackup:()=>invoke<BackupInfo>("create_backup"),
  importBackup:(path:string)=>invoke<BackupInfo>("import_backup",{path}),
  openBackupDirectory:()=>invoke<void>("open_backup_directory"),
  mcpConnectionGuide:()=>invoke<string>("mcp_connection_guide"),
  restoreBackup:(path:string)=>invoke<BackupMergeResult>("restore_backup",{path}),
  deleteBackup:(path:string)=>invoke<void>("delete_backup",{path}),
  setSetting:(key:"show_deferred_in_queue"|"week_start_day"|"statistics_rate_mode"|"launch_at_login",value:boolean|string)=>invoke<void>("set_setting",{key,value:typeof value==="boolean"?(value?"true":"false"):value}),
  toggleFloating:()=>invoke<boolean>("toggle_floating"),
  showMain:()=>invoke<void>("show_main_window"),
  requestNewTask:()=>invoke<void>("request_new_task"),
  globalShortcutAvailable:()=>invoke<boolean>("global_shortcut_available"),
  setGlobalShortcut:(shortcut:string)=>invoke<void>("set_global_shortcut",{shortcut}),
  saveChartExport:(path:string,bytes:Uint8Array)=>invoke<void>("save_chart_export",{path,bytes:Array.from(bytes)}),
  openTaskAction:(id:number,action:TaskUiAction["action"]|"complete"|"archive"|"delete"|"restore")=>invoke<void>("open_task_action",{request:{id,action}}),
  getTask:(id:number)=>invoke<LegalTask>("copy_ticket_card",{id}),
  getVersion,
  copyText:(value:string)=>writeText(value),
  onDataChanged:(callback:()=>void)=>{
    let dispose:(()=>void)|undefined;
    let disposed=false;
    void listen("data-changed",callback).then((value)=>{
      if(disposed)value();else dispose=value;
    });
    return()=>{disposed=true;dispose?.();};
  },
  onNewTask:(callback:()=>void)=>{
    let dispose:(()=>void)|undefined;
    let disposed=false;
    void listen("new-task",callback).then((value)=>{
      if(disposed)value();else dispose=value;
    });
    return()=>{disposed=true;dispose?.();};
  },
  onTaskUiAction:(callback:(action:TaskUiAction)=>void)=>{
    let dispose:(()=>void)|undefined;
    let disposed=false;
    void listen<TaskUiAction>("task-ui-action",event=>callback(event.payload)).then((value)=>{
      if(disposed)value();else dispose=value;
    });
    return()=>{disposed=true;dispose?.();};
  },
  copyTicketImage:async(taskOrId:LegalTask|number)=>{
    const id=typeof taskOrId==="number"?taskOrId:taskOrId.id;
    const snapshot=await invoke<TicketSnapshot>("ticket_snapshot",{id});
    let image:Image;
    if(pngImageSupported){
      try{
        image=await Image.fromBytes(await renderTicketPng(snapshot.task,snapshot.queueAhead));
      }catch{
        pngImageSupported=false;
        const rendered=await renderTicketRgba(snapshot.task,snapshot.queueAhead);
        image=await Image.new(rendered.rgba,rendered.width,rendered.height);
      }
    }else{
      const rendered=await renderTicketRgba(snapshot.task,snapshot.queueAhead);
      image=await Image.new(rendered.rgba,rendered.width,rendered.height);
    }
    try{
      await writeImage(image);
    }finally{
      await image.close();
    }
    return snapshot.task;
  },
  setLaunchAtLogin:(enabled:boolean)=>invoke<void>("set_launch_at_login",{enabled}),
  launchAtLogin:()=>invoke<boolean>("get_launch_at_login")
};
