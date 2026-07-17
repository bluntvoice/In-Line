import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import { Image } from "@tauri-apps/api/image";
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { writeImage,writeText } from "@tauri-apps/plugin-clipboard-manager";
import type { BackupInfo,BootstrapData,LegalTask,MasterData,MoveDirection,TaskInput,TaskLog,TaskStatus,TaskUiAction,TaskView } from "./types";
import { renderTicketRgba } from "./lib/ticket-image";

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
  archiveTask:(id:number)=>invoke<void>("archive_task",{id}),
  getLogs:(taskId:number)=>invoke<TaskLog[]>("get_logs",{taskId}),
  addLog:(taskId:number,content:string)=>invoke<void>("add_log",{taskId,content}),
  addMaster:(kind:"department"|"task_type"|"contact",name:string)=>invoke<MasterData>("add_master",{kind,name}),
  listBackups:()=>invoke<BackupInfo[]>("list_backups"),
  createBackup:()=>invoke<BackupInfo>("create_backup"),
  restoreBackup:(path:string)=>invoke<void>("restore_backup",{path}),
  toggleFloating:()=>invoke<boolean>("toggle_floating"),
  showMain:()=>invoke<void>("show_main_window"),
  requestNewTask:()=>invoke<void>("request_new_task"),
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
    const task=typeof taskOrId==="number"?await invoke<LegalTask>("copy_ticket_card",{id:taskOrId}):taskOrId;
    const rendered=await renderTicketRgba(task);
    const image=await Image.new(rendered.rgba,rendered.width,rendered.height);
    try{
      await writeImage(image);
    }finally{
      await image.close();
    }
    return task;
  },
  registerNewTaskShortcut:async(callback:()=>void)=>{
    await unregister("Ctrl+Alt+N").catch(()=>undefined);
    await register("Ctrl+Alt+N",callback);
    return()=>{void unregister("Ctrl+Alt+N");};
  },
  setLaunchAtLogin:async(value:boolean)=>{if(value)await enable();else await disable();},
  launchAtLogin:isEnabled
};
