import {describe,expect,it} from "vitest";
import type {StatisticsDetail} from "../types";
import {buildStatisticsDetailsCopy} from "./statistics-copy";

const detail=(taskId:number,taskType:string,department:string,title:string,hasProcessedOrCompleted=true):StatisticsDetail=>({
  taskId,taskType,department,title,hasProcessedOrCompleted,resultStatus:"waiting_materials",permanentNumber:"IL-"+taskId,
  contact:"",firstHandledAt:"",lastHandledAt:"",handlingCount:3
});

describe("copy statistics details",()=>{
  it("includes previously processed tasks even if their latest result is waiting, but excludes waiting-only tasks",()=>{
    expect(buildStatisticsDetailsCopy([detail(1,"合同","团队","已处理后等待"),detail(2,"合同","团队","仅等待",false)])).toEqual({count:1,text:"合同-团队-已处理后等待\n"});
  });
  it("sorts by type, department then title, using Chinese and natural numeric ordering",()=>{
    const input=[detail(1,"咨询","团队1","事项1"),detail(2,"合同","团队2","事项1"),detail(3,"合同","团队1","事项10"),detail(4,"合同","团队1","事项2")];
    expect(buildStatisticsDetailsCopy(input).text).toBe("合同-团队1-事项2\n合同-团队1-事项10\n合同-团队2-事项1\n咨询-团队1-事项1\n");
    expect(input.map(item=>item.taskId)).toEqual([1,2,3,4]);
  });
  it("preserves multiple departments and separate identically worded tasks, with exactly one line per item",()=>{
    expect(buildStatisticsDetailsCopy([detail(1,"合同","部门一、部门二","甲\r\n乙"),detail(2,"合同","部门一、部门二","甲\n乙")])).toEqual({count:2,text:"合同-部门一、部门二-甲 乙\n合同-部门一、部门二-甲 乙\n"});
  });
  it("returns an empty copy without a stray newline when no task is eligible",()=>{
    expect(buildStatisticsDetailsCopy([])).toEqual({count:0,text:""});
    expect(buildStatisticsDetailsCopy([detail(1,"合同","团队","仅等待",false)])).toEqual({count:0,text:""});
  });
});
