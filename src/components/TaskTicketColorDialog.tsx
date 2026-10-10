import { useEffect, useRef, useState } from "react";
import { Check, RotateCcw, X } from "lucide-react";
import { api } from "../api";
import type { LegalTask } from "../types";
import { displayTicket } from "../lib/task-utils";
import { normalizeHexColor, OFFICIAL_TICKET_COLORS, ticketTextColor } from "../lib/ticket-colors";

export default function TaskTicketColorDialog({task,onClose,onSaved}:{task:LegalTask;onClose:()=>void;onSaved:()=>void}) {
  const [draft,setDraft]=useState(task.ticketColor??"");
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const locked=useRef(false);
  const dialog=useRef<HTMLElement>(null);
  const preview=normalizeHexColor(draft)??task.ticketColor??"#0B3A82";
  useEffect(()=>{
    const previous=document.activeElement as HTMLElement|null;
    dialog.current?.querySelector<HTMLInputElement>("input")?.focus();
    const key=(event:KeyboardEvent)=>{
      if(event.key==="Escape"&&!locked.current){event.preventDefault();event.stopPropagation();onClose();}
      if(event.key==="Tab"){
        const controls=Array.from(dialog.current?.querySelectorAll<HTMLElement>("button:not(:disabled),input:not(:disabled)")??[]);
        const first=controls[0],last=controls.at(-1);
        if(event.shiftKey&&document.activeElement===first){event.preventDefault();last?.focus();}
        else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first?.focus();}
      }
    };
    document.addEventListener("keydown",key,true);
    return()=>{document.removeEventListener("keydown",key,true);previous?.isConnected&&previous.focus();};
  },[onClose]);
  const save=async(color:string|null)=>{
    if(locked.current)return;
    const normalized=color===null?null:normalizeHexColor(color);
    if(color!==null&&!normalized){setError("请输入3位或6位十六进制颜色，例如 #0B3A82");return;}
    locked.current=true;setBusy(true);setError("");
    try{await api.setTaskTicketColor(task.id,normalized);onSaved();onClose();}
    catch(reason){setError("保存失败："+String(reason));}
    finally{locked.current=false;setBusy(false);}
  };
  return <div className="modal-layer nested-modal" onMouseDown={event=>{if(event.target===event.currentTarget&&!locked.current)onClose();}}>
    <section className="ticket-color-picker task-ticket-color-picker" ref={dialog} role="dialog" aria-modal="true" aria-labelledby="task-ticket-color-title">
      <header><div><h2 id="task-ticket-color-title">事项编号配色</h2><p>{task.title}</p></div><button type="button" className="icon-button" aria-label="关闭编号配色" disabled={busy} onClick={onClose}><X size={18}/></button></header>
      <p className="task-ticket-color-note">仅修改此事项。逾期或最高优先级警示时显示红色，警示解除后恢复自选色。</p>
      <form className="task-ticket-color-code" onSubmit={event=>{event.preventDefault();void save(draft);}}>
        <span className="ticket-color-preview" style={{backgroundColor:preview,color:ticketTextColor(preview)}}>{displayTicket(task)}</span>
        <label>HEX 编码<input aria-label="事项编号 HEX 编码" value={draft} onChange={event=>{setDraft(event.target.value);setError("");}} maxLength={16} disabled={busy} spellCheck={false}/></label>
        <button className="button secondary small" type="submit" disabled={busy}>应用</button>
      </form>
      {error&&<p className="ticket-color-error" role="alert">{error}</p>}
      <div className="ticket-color-options" role="group" aria-label="事项编号候选色">{OFFICIAL_TICKET_COLORS.map(color=><button type="button" key={color.hex} disabled={busy} aria-label={`${color.name} ${color.hex}`} aria-pressed={task.ticketColor===color.hex} onClick={()=>void save(color.hex)}>
        <span className="ticket-color-preview" style={{backgroundColor:color.hex,color:ticketTextColor(color.hex)}}>01</span><strong>{color.name}{task.ticketColor===color.hex&&<Check size={13}/>}</strong><small>{color.hex}</small>
      </button>)}</div>
      <footer><button type="button" className="button secondary small" disabled={busy} onClick={()=>void save(null)}><RotateCcw size={15}/>恢复自动配色</button><button type="button" className="button secondary small" disabled={busy} onClick={onClose}>取消</button></footer>
    </section>
  </div>;
}
