import { describe, expect, it } from "vitest";
import type { LegalTask } from "../types";
import { taskTicketColor } from "./task-ticket-color";
import TicketNumber from "../components/TicketNumber";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";

const now=new Date("2026-10-10T12:00:00+08:00");
const task={id:1,status:"pending",priority:"normal",requestedDeadline:null,ticketColor:"#abc",ticketDate:"2026-10-10",dailySequence:1} as LegalTask;
describe("task ticket color priority",()=>{
  it("retains a normalized custom color for urgent and scheduled tasks",()=>{
    expect(taskTicketColor({...task,isUrgent:true,isScheduled:true} as LegalTask,now)).toBe("#AABBCC");
    expect(taskTicketColor({...task,ticketColor:null},now)).toBeNull();
    expect(taskTicketColor({...task,ticketColor:"url(x)"},now)).toBeNull();
  });
  it("temporarily overrides with warning red and restores after the warning ends",()=>{
    const overdue={...task,requestedDeadline:"2026-10-09T12:00:00+08:00"};
    expect(taskTicketColor(overdue,now)).toBe("#C43D4B");
    expect(taskTicketColor({...overdue,status:"completed"},now)).toBe("#AABBCC");
    expect(taskTicketColor({...overdue,requestedDeadline:"2026-10-11T12:00:00+08:00"},now)).toBe("#AABBCC");
    expect(taskTicketColor({...task,priority:"critical"},now)).toBe("#C43D4B");
    expect(taskTicketColor({...task,priority:"normal"},now)).toBe("#AABBCC");
  });
  it("renders the shared ticket component with readable custom and warning colors",()=>{
    const markup=renderToStaticMarkup(createElement(TicketNumber,{task:{...task,ticketColor:"#F3D98B",isUrgent:true,isScheduled:true}}));
    expect(markup).toContain("background-color:#F3D98B;color:#000000");
    const alert=renderToStaticMarkup(createElement(TicketNumber,{task:{...task,priority:"critical"}}));
    expect(alert).toContain("background-color:#C43D4B;color:#FFFFFF");
    expect(alert).toContain("时效警示");
  });
});
