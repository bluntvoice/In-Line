import { useEffect,useRef } from "react";

export default function ColumnResizeHandle({label,width,minWidth,maxWidth,onResize}:{label:string;width:number;minWidth:number;maxWidth:number;onResize:(width:number)=>void}){
  const cleanupRef=useRef<(()=>void)|null>(null);
  useEffect(()=>()=>cleanupRef.current?.(),[]);
  const clamp=(value:number)=>Math.min(maxWidth,Math.max(minWidth,Math.round(value)));
  const start=(event:React.PointerEvent<HTMLSpanElement>)=>{
    if(event.button!==0)return;
    event.preventDefault();event.stopPropagation();cleanupRef.current?.();
    const startX=event.clientX,startWidth=width;
    const previousCursor=document.body.style.cursor,previousSelect=document.body.style.userSelect;
    document.body.style.cursor="col-resize";document.body.style.userSelect="none";
    const move=(moveEvent:PointerEvent)=>onResize(clamp(startWidth+moveEvent.clientX-startX));
    const cleanup=()=>{window.removeEventListener("pointermove",move);window.removeEventListener("pointerup",cleanup);window.removeEventListener("pointercancel",cleanup);document.body.style.cursor=previousCursor;document.body.style.userSelect=previousSelect;cleanupRef.current=null;};
    cleanupRef.current=cleanup;window.addEventListener("pointermove",move);window.addEventListener("pointerup",cleanup,{once:true});window.addEventListener("pointercancel",cleanup,{once:true});
  };
  return <span className="column-resize-handle" role="separator" aria-orientation="vertical" aria-label={`调整${label}列宽`} tabIndex={0} onPointerDown={start} onClick={event=>event.stopPropagation()} onKeyDown={event=>{if(event.key!=="ArrowLeft"&&event.key!=="ArrowRight")return;event.preventDefault();event.stopPropagation();onResize(clamp(width+(event.key==="ArrowRight"?8:-8)));}}/>;
}
