export type TaskStatus="pending"|"processing"|"waiting_materials"|"waiting_confirmation"|"paused"|"completed"|"cancelled"|"archived";
export type Priority="normal"|"elevated"|"urgent"|"critical";
export type Workload="simple"|"standard"|"complex"|"major";
export type TaskView="queue"|"archive"|"trash";
export type MoveDirection="up"|"down";

export interface LegalTask{
  id:number;permanentNumber:string;dailySequence:number;ticketDate:string;department:string;contact:string;
  taskType:string;title:string;details:string;status:TaskStatus;priority:Priority;workload:Workload;isUrgent:boolean;
  urgentRequester:string;urgentReason:string;requestedDeadline:string|null;internalNotes:string;createdAt:string;
  updatedAt:string;startedAt:string|null;completedAt:string|null;archivedAt:string|null;deletedAt:string|null;customSortOrder:number;
}
export interface TaskInput{
  id?:number;department:string;contact:string;taskType:string;title:string;details:string;status:TaskStatus;priority:Priority;
  workload:Workload;isUrgent:boolean;urgentRequester:string;urgentReason:string;requestedDeadline:string|null;internalNotes:string;
}
export interface TaskLog{id:number;taskId:number;logType:string;content:string;createdAt:string}
export interface MasterData{departments:string[];taskTypes:string[]}
export interface BackupInfo{name:string;path:string;size:number;modifiedAt:string}
export interface TaskUiAction{id:number;action:"view"|"edit"|"status"|"urgent"}
export interface BootstrapData{
  queue:LegalTask[];archive:LegalTask[];trash:LegalTask[];masters:MasterData;settings:Record<string,string>;backups:BackupInfo[];
}
