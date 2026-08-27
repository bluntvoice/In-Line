export type TaskStatus="pending"|"processing"|"waiting_materials"|"waiting_confirmation"|"waiting_counterparty_confirmation"|"paused"|"processed"|"completed"|"cancelled"|"archived";
export type Priority="normal"|"elevated"|"urgent"|"critical";
export type Workload="simple"|"standard"|"complex"|"major";
export type TaskView="queue"|"archive"|"trash";
export type MoveDirection="up"|"down";

export interface LegalTask{
  id:number;permanentNumber:string;dailySequence:number;ticketDate:string;department:string;departments:string[];contact:string;contacts:string[];
  taskType:string;title:string;details:string;status:TaskStatus;priority:Priority;workload:Workload;isUrgent:boolean;
  urgentRequester:string;urgentReason:string;requestedDeadline:string|null;requestedDeadlineLabel:string|null;internalNotes:string;createdAt:string;
  updatedAt:string;startedAt:string|null;completedAt:string|null;archivedAt:string|null;deletedAt:string|null;customSortOrder:number;
  processingRounds:number;hasActiveQueue:boolean;deferredEnteredAt:string|null;isImportConflict:boolean;
}
export interface TaskInput{
  id?:number;department:string;departments:string[];contact:string;contacts:string[];taskType:string;title:string;details:string;status:TaskStatus;priority:Priority;
  workload:Workload;isUrgent:boolean;urgentRequester:string;urgentReason:string;requestedDeadline:string|null;requestedDeadlineLabel:string|null;internalNotes:string;
}
export interface TaskLog{id:number;taskId:number;logType:string;content:string;createdAt:string}
export interface TaskWorkEvent{id:number;taskId:number;resultStatus:WorkResult;handledAt:string;taskTypeSnapshot:string;source:string;note:string;createdAt:string;updatedAt:string;isFirstValid:boolean}
export type WorkResult="processed"|"completed"|"waiting_materials"|"waiting_confirmation"|"waiting_counterparty_confirmation";
export interface QueueInput{id:number;inheritDeadline:boolean;reason:string}
export interface MergeTaskInput{targetTaskId:number;sourceTaskId:number;deduplicateRecords:boolean;trashSource:boolean}
export interface TicketSnapshot{task:LegalTask;queueAhead:number}
export interface MasterData{departments:string[];taskTypes:string[];contacts:string[]}
export interface BackupInfo{name:string;path:string;size:number;modifiedAt:string}
export interface BackupConflictItem{taskId:number;permanentNumber:string;sourceTitle:string;importedTitle:string}
export interface BackupMergeResult{addedTasks:number;mergedTasks:number;conflictTasks:number;appliedSettings:number;conflicts:BackupConflictItem[]}
export interface TaskUiAction{id:number;action:"view"|"edit"|"status"|"urgent"}
export interface BootstrapData{
  queue:LegalTask[];archive:LegalTask[];trash:LegalTask[];masters:MasterData;settings:Record<string,string>;backups:BackupInfo[];
}
export interface StatisticsResult{
  range:{start:string;end:string};
  summary:{handledTasks:number;processed:number;completed:number;waitingMaterials:number;waitingConfirmation:number;waitingCounterpartyConfirmation:number;rateMode:"closure"|"processing";rateNumerator:number;rateDenominator:number;completionRate:number};
  byTaskType:Array<{taskType:string;handledTasks:number;completed:number;pendingFollowUp:number}>;
  byDepartment:Array<{department:string;handledTasks:number;completed:number;pendingFollowUp:number}>;
  trend:Array<{periodStart:string;handledTasks:number}>;
  trendGranularity:"day"|"week";
}
export interface StatisticsDetail{taskId:number;permanentNumber:string;title:string;department:string;contact:string;resultStatus:WorkResult;firstHandledAt:string;lastHandledAt:string;handlingCount:number}
export interface WorkCalendarInterval{queueEntryId:number;enqueuedAt:string;closedAt:string|null;roundIndex:number;resultStatus:WorkResult|null;handledAt:string|null;currentActive:boolean}
export interface WorkCalendarTask{taskId:number;permanentNumber:string;title:string;taskType:string;intervals:WorkCalendarInterval[]}
export interface WorkCalendarEvent{eventId:number;taskId:number;permanentNumber:string;title:string;taskType:string;resultStatus:WorkResult;handledAt:string;roundIndex:number|null}
export interface WorkCalendarResult{
  range:{start:string;end:string;generatedAt:string};
  summary:{handledTasks:number;handlingRounds:number;completedTasks:number};
  tasks:WorkCalendarTask[];
  events:WorkCalendarEvent[];
}
