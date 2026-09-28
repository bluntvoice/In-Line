import { describe,expect,it } from "vitest";
import type { LegalTask } from "../types";
import { groupSubtasks,needsParentCompletionChoice,shouldOfferParentCompletion,subtaskProgress } from "./subtask-progress";

const task=(id:number,parentTaskId:number|null,status:LegalTask["status"],subtaskSortOrder=0)=>({id,parentTaskId,status,subtaskSortOrder}) as LegalTask;

describe("subtask progress",()=>{
  it("counts every existing child but only completed status in the numerator",()=>{
    expect(subtaskProgress([
      task(2,1,"completed"),
      task(3,1,"processed"),
      task(4,1,"cancelled"),
      task(5,1,"archived")
    ])).toEqual({total:4,completed:1,percentage:25});
  });

  it("groups and orders children without mixing queue order into relationship order",()=>{
    const grouped=groupSubtasks([
      task(3,1,"pending",2),task(2,1,"pending",1),task(5,4,"pending",1),task(1,null,"pending")
    ]);
    expect(grouped.get(1)?.map(item=>item.id)).toEqual([2,3]);
    expect(grouped.get(4)?.map(item=>item.id)).toEqual([5]);
  });

  it("distinguishes the parent choice from the last-child completion hint",()=>{
    const state={parentTaskId:1,totalSubtasks:2,completedSubtasks:1,eligibleSubtasks:2,completedEligibleSubtasks:1,allEligibleSubtasksCompleted:false,parentCanBeCompleted:true};
    expect(needsParentCompletionChoice(1,state)).toBe(true);
    expect(needsParentCompletionChoice(2,state)).toBe(false);
    expect(shouldOfferParentCompletion(2,{...state,completedSubtasks:2,completedEligibleSubtasks:2,allEligibleSubtasksCompleted:true})).toBe(true);
    expect(shouldOfferParentCompletion(1,{...state,allEligibleSubtasksCompleted:true})).toBe(false);
  });
});
