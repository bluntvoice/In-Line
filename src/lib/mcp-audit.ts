export interface McpAudit { id:number;clientId:string;action:string;taskId:number|null;reason:string;intent:string;createdAt:string;before:unknown;after:unknown;undoOf:number|null }
export interface McpUndo {id:number;auditId:number;reason:string;createdAt:string}
export interface McpAuditState {audits:McpAudit[];pending:McpUndo[]}
export function auditLabel(action:string){return ({create:"新建事项",createSubtask:"创建子任务",cliCompleteGroup:"完成父子事项",setParent:"调整所属任务",reorderSubtasks:"调整子任务顺序",queue:"调整队列",reschedule:"调整加入日期",archive:"归档事项",trash:"移至回收站",restore:"恢复事项",correctWorkEvent:"更正办理",voidWorkEvent:"作废办理",patch:"编辑事项",setStatus:"调整状态",setUrgent:"调整加急",recordWorkEvent:"记录办理",preferences:"调整偏好",approvedUndo:"批准撤销"} as Record<string,string>)[action]??action;}
export function auditDiff(audit:McpAudit){
  const unwrap=(value:unknown):Record<string,unknown>=>{if(!value||typeof value!=="object")return {};const record=value as Record<string,unknown>;return record.task&&typeof record.task==="object"?record.task as Record<string,unknown>:record;};
  const before=unwrap(audit.before),after=unwrap(audit.after);
  const changes=[...new Set([...Object.keys(before),...Object.keys(after)])].filter(key=>key!=="fieldVersions"&&JSON.stringify(before[key])!==JSON.stringify(after[key])).map(key=>({key,before:before[key],after:after[key]}));
  const original=(audit.before as {relatedTasks?:Record<string,unknown>[]} | null)?.relatedTasks??[];
  const related=(audit.after as {relatedTasks?:{task:Record<string,unknown>}[]} | null)?.relatedTasks??[];
  related.forEach(row=>{const old=original.find(t=>t.id===row.task.id);
    if(audit.action==="cliCompleteGroup"){
      for(const key of new Set([...Object.keys(old??{}),...Object.keys(row.task)])){
        if(JSON.stringify(old?.[key])!==JSON.stringify(row.task[key]))changes.push({key:`事项 ${row.task.id} · ${key}`,before:old?.[key],after:row.task[key]});
      }
    }else changes.push({key:`事项 ${row.task.id} 的队列顺序`,before:old?.customSortOrder,after:row.task.customSortOrder});});
  return changes;
}
