import { describe,expect,it } from "vitest";
import type { WorkCalendarResult } from "../types";
import { calendarRange,clipInterval,monthGrid,visibleWeekDays } from "./work-calendar";

const empty=(generatedAt="2026-08-27T12:00:00+08:00"):WorkCalendarResult=>({range:{start:"2026-08-24T00:00:00+08:00",end:"2026-08-31T00:00:00+08:00",generatedAt},summary:{handledTasks:0,handlingRounds:0,completedTasks:0},tasks:[],events:[]});

describe("work calendar date layout",()=>{
  it("respects Monday and Sunday week starts",()=>{
    expect(calendarRange(new Date(2026,7,27),"week","monday").start.getDay()).toBe(1);
    expect(calendarRange(new Date(2026,7,27),"week","sunday").start.getDay()).toBe(0);
  });
  it("hides empty weekends and reveals weekends with actual records",()=>{
    const start=new Date(2026,7,24);expect(visibleWeekDays(empty(),start)).toHaveLength(5);
    const data=empty();data.events.push({eventId:1,taskId:1,permanentNumber:"1",title:"周六事项",taskType:"测试",resultStatus:"processed",handledAt:"2026-08-29T10:00:00+08:00",roundIndex:1});
    expect(visibleWeekDays(data,start).map(day=>day.getDay())).toEqual([1,2,3,4,5,6]);
    const queueOnly=empty();queueOnly.tasks.push({taskId:2,permanentNumber:"2",title:"周日区间",taskType:"测试",intervals:[{queueEntryId:2,enqueuedAt:"2026-08-30T09:00:00+08:00",closedAt:"2026-08-30T11:00:00+08:00",roundIndex:1,resultStatus:null,handledAt:null,currentActive:false}]});
    expect(visibleWeekDays(queueOnly,start).map(day=>day.getDay())).toEqual([1,2,3,4,5,0]);
  });
  it("keeps cross-period intervals proportional after clipping",()=>{
    const clipped=clipInterval("2026-08-23T12:00:00+08:00","2026-08-25T12:00:00+08:00",new Date("2026-08-24T00:00:00+08:00"),new Date("2026-08-31T00:00:00+08:00"),"2026-08-27T12:00:00+08:00");
    expect(clipped.clippedStart).toBe(true);expect(clipped.left).toBe(0);expect(clipped.width).toBeCloseTo(21.43,1);
  });
  it("builds a stable traditional month grid",()=>{
    const days=monthGrid(new Date(2026,7,1),"monday");expect(days.length).toBeGreaterThanOrEqual(35);expect(days.length%7).toBe(0);expect(days[0].getDay()).toBe(1);
  });
});
