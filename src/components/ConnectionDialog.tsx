import {useEffect,useRef,type ReactNode} from "react";
import {X} from "lucide-react";

// Native modal dialog supplies focus containment and returns focus to its opener.
export default function ConnectionDialog({title,busy,onClose,children}:{title:string;busy:boolean;onClose:()=>void;children:ReactNode}){
  const dialog=useRef<HTMLDialogElement>(null);
  useEffect(()=>{const element=dialog.current;element?.showModal();return()=>element?.close();},[]);
  return <dialog ref={dialog} className="mcp-connect-dialog" aria-labelledby="connection-dialog-title" onCancel={event=>{event.preventDefault();if(!busy)onClose();}}>
    <header className="modal-header"><h2 id="connection-dialog-title">{title}</h2><button className="icon-button" disabled={busy} onClick={onClose} aria-label="关闭接入窗口"><X size={18}/></button></header>
    <div className="modal-body">{children}</div>
  </dialog>;
}
