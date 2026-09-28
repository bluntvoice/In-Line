import type { LegalTask,SubtaskCompletionState } from "../types";

export interface SubtaskProgressValue {
  total:number;
  handled:number;
  percentage:number;
}

export function groupSubtasks(tasks:LegalTask[]) {
  const groups=new Map<number,LegalTask[]>();
  for(const task of tasks){
    if(task.parentTaskId===null)continue;
    const group=groups.get(task.parentTaskId)??[];
    group.push(task);
    groups.set(task.parentTaskId,group);
  }
  for(const group of groups.values())group.sort((a,b)=>a.subtaskSortOrder-b.subtaskSortOrder||a.id-b.id);
  return groups;
}

export function subtaskProgress(subtasks:Pick<LegalTask,"status">[]):SubtaskProgressValue {
  const total=subtasks.length;
  const handled=subtasks.filter(task=>task.status==="completed"||task.status==="processed").length;
  return {total,handled,percentage:total?Math.round(handled/total*100):0};
}

export function needsParentCompletionChoice(taskId:number,state:SubtaskCompletionState|null):state is SubtaskCompletionState {
  return Boolean(state&&state.parentTaskId===taskId&&state.eligibleSubtasks>state.completedEligibleSubtasks);
}

export function shouldOfferParentCompletion(completedTaskId:number,state:SubtaskCompletionState|null):state is SubtaskCompletionState {
  return Boolean(state&&state.parentTaskId!==completedTaskId&&state.allEligibleSubtasksCompleted&&state.parentCanBeCompleted);
}
