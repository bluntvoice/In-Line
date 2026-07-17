import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { writeImage } from "@tauri-apps/plugin-clipboard-manager";
import type { BackupInfo,BootstrapData,LegalTask,MasterData,MoveDirection,TaskInput,TaskLog,TaskStatus,TaskView } from "./types";
import { renderTicketPng } from "./lib/ticket-image";

export const api={
  bootstrap:()=>invoke<BootstrapData>("bootstrap"),
  listTasks:(view:TaskView)=>invoke<LegalTask[]>("list_tasks",{view}),
  saveTask:(task:TaskInput)=>invoke<LegalTask>("save_task",{task}),
  setTaskStatus:(id:number,status:TaskStatus)=>invoke<void>("set_task_status",{id,status}),
  moveTask:(id:number,direction:MoveDirection)=>invoke<void>("move_task",{id,direction}),
  deleteTask:(id:number)=>invoke<void>("delete_task",{id}),
  restoreTask:(id:number)=>invoke<void>("restore_task",{id}),
  archiveTask:(id:number)=>invoke<void>("archive_task",{id}),
  getLogs:(taskId:number)=>invoke<TaskLog[]>("get_logs",{taskId}),
  addLog:(taskId:number,content:string)=>invoke<void>("add_log",{taskId,content}),
  addMaster:(kind:"department"|"task_type",name:string)=>invoke<MasterData>("add_master",{kind,name}),
  listBackups:()=>invoke<BackupInfo[]>("list_backups"),
  createBackup:()=>invoke<BackupInfo>("create_backup"),
  restoreBackup:(path:string)=>invoke<void>("restore_backup",{path}),
  toggleFloating:()=>invoke<boolean>("toggle_floating"),
  showMain:()=>invoke<void>("show_main_window"),
  requestNewTask:()=>invoke<void>("request_new_task"),
  onDataChanged:(callback:()=>void)=>{
    let dispose:(()=>void)|undefined;
    void listen("data-changed",callback).then((value)=>{dispose=value;});
    return()=>dispose?.();
  },
  onNewTask:(callback:()=>void)=>{
    let dispose:(()=>void)|undefined;
    void listen("new-task",callback).then((value)=>{dispose=value;});
    return()=>dispose?.();
  },
  copyTicketImage:async(taskOrId:LegalTask|number)=>{
    const task=typeof taskOrId==="number"?await invoke<LegalTask>("copy_ticket_card",{id:taskOrId}):taskOrId;
    const bytes=await renderTicketPng(task);
    await writeImage(bytes);
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
