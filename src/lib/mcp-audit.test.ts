import {describe,it,expect} from "vitest";
import {auditDiff,auditLabel} from "./mcp-audit";
import type {McpAudit} from "./mcp-audit";
const base:McpAudit={id:1,clientId:"synthetic",action:"patch",taskId:1,reason:"user request",intent:"synthetic",createdAt:"2026-10-10",before:null,after:null,undoOf:null};
describe("AI audit review",()=>{
  it("makes parent and child completion changes visible before local undo approval",()=>{
    expect(auditLabel("cliCompleteGroup")).toBe("完成父子事项");expect(auditLabel("createSubtask")).toBe("创建子任务");
    const diff=auditDiff({...base,action:"cliCompleteGroup",before:{task:{id:1,status:"pending"},relatedTasks:[{id:2,status:"pending",hasActiveQueue:true}]},after:{task:{id:1,status:"completed"},relatedTasks:[{task:{id:2,status:"completed",hasActiveQueue:false}}]}});
    expect(diff).toContainEqual({key:"事项 2 · status",before:"pending",after:"completed"});
    expect(diff).toContainEqual({key:"事项 2 · hasActiveQueue",before:true,after:false});
  });
  it("shows task changed fields and exact before/after while retaining empty values",()=>{expect(auditDiff({...base,before:{task:{title:"before",details:"",status:"pending"}},after:{task:{title:"after",details:"text",status:"pending"}}})).toEqual([{key:"title",before:"before",after:"after"},{key:"details",before:"",after:"text"}]);});
  it("shows missing preference and creation values without inventing data",()=>{expect(auditDiff({...base,before:{ui_scale:null},after:{ui_scale:"120"}})).toEqual([{key:"ui_scale",before:null,after:"120"}]);expect(auditDiff({...base,before:null,after:{task:{title:"new"}}})).toEqual([{key:"title",before:undefined,after:"new"}]);});
  it("labels approved undo separately from real work",()=>{expect(auditLabel("approvedUndo")).toBe("批准撤销");expect(auditLabel("recordWorkEvent")).toBe("记录办理");});
});
