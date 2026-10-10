export interface McpAudit { id:number;clientId:string;action:string;taskId:number|null;reason:string;intent:string;createdAt:string;before:unknown;after:unknown;undoOf:number|null }
export interface McpUndo {id:number;auditId:number;reason:string;createdAt:string}
export interface McpAuditState {audits:McpAudit[];pending:McpUndo[]}
export function auditLabel(action:string){return ({create:"新建事项",patch:"编辑事项",setStatus:"调整状态",setUrgent:"调整加急",recordWorkEvent:"记录办理",preferences:"调整偏好",approvedUndo:"批准撤销"} as Record<string,string>)[action]??action;}
export function auditDiff(audit:McpAudit){
  const unwrap=(value:unknown):Record<string,unknown>=>{if(!value||typeof value!=="object")return {};const record=value as Record<string,unknown>;return record.task&&typeof record.task==="object"?record.task as Record<string,unknown>:record;};
  const before=unwrap(audit.before),after=unwrap(audit.after);
  const changes=[...new Set([...Object.keys(before),...Object.keys(after)])].filter(key=>key!=="fieldVersions"&&JSON.stringify(before[key])!==JSON.stringify(after[key])).map(key=>({key,before:before[key],after:after[key]}));
  const original=(audit.before as {relatedTasks?:Record<string,unknown>[]} | null)?.relatedTasks??[];
  const related=(audit.after as {relatedTasks?:{task:Record<string,unknown>}[]} | null)?.relatedTasks??[];
  related.forEach(row=>{const old=original.find(t=>t.id===row.task.id);changes.push({key:`事项 ${row.task.id} 的队列顺序`,before:old?.customSortOrder,after:row.task.customSortOrder});});
  return changes;
}
