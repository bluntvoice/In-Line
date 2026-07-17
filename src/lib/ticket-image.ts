import type { LegalTask } from "../types";
import { displayTicket,STATUS_LABELS } from "./task-utils";

const WIDTH=1200,HEIGHT=800;
const COLORS={blue:"#0B3A82",ink:"#102A56",amber:"#FFB000",paper:"#F7F9FC",muted:"#526173",red:"#C43D4B"};

function rounded(context:CanvasRenderingContext2D,x:number,y:number,w:number,h:number,r:number,fill:string){
  context.beginPath();context.roundRect(x,y,w,h,r);context.fillStyle=fill;context.fill();
}
function fitLines(context:CanvasRenderingContext2D,text:string,maxWidth:number,maxLines=2){
  const lines:string[]=[];let line="";
  for(const char of text){
    const next=line+char;
    if(line&&context.measureText(next).width>maxWidth){lines.push(line);line=char;if(lines.length===maxLines-1)break;}else line=next;
  }
  if(line&&lines.length<maxLines)lines.push(line);
  if(lines.join("").length<text.length){
    let last=lines.at(-1)??"";
    while(last&&context.measureText(last+"…").width>maxWidth)last=last.slice(0,-1);
    lines[lines.length-1]=last+"…";
  }
  return lines;
}
function dateTime(value:string|null){if(!value)return"未设置";return new Intl.DateTimeFormat("zh-CN",{month:"2-digit",day:"2-digit",hour:"2-digit",minute:"2-digit",hour12:false}).format(new Date(value));}

export async function renderTicketPng(task:LegalTask):Promise<Uint8Array>{
  await document.fonts.ready;
  const canvas=document.createElement("canvas");canvas.width=WIDTH;canvas.height=HEIGHT;
  const context=canvas.getContext("2d");if(!context)throw new Error("当前设备无法生成取号图片");
  context.textBaseline="middle";context.fillStyle=COLORS.paper;context.fillRect(0,0,WIDTH,HEIGHT);
  rounded(context,36,36,1128,728,34,"#FFFFFF");
  context.fillStyle=COLORS.blue;context.fillRect(36,36,18,728);
  context.font="700 28px 'Microsoft YaHei UI','Microsoft YaHei',sans-serif";context.fillStyle=COLORS.ink;context.fillText("IN LINE",90,92);
  context.font="500 22px 'Microsoft YaHei UI','Microsoft YaHei',sans-serif";context.fillStyle=COLORS.muted;context.fillText("事项已登记，请按队列处理",90,132);
  const alert=task.isUrgent||task.priority==="critical";
  rounded(context,alert?928:968,74,alert?166:126,50,25,alert?COLORS.red:COLORS.blue);
  context.font="700 22px 'Microsoft YaHei UI','Microsoft YaHei',sans-serif";context.fillStyle="#FFF";context.textAlign="center";
  context.fillText(alert?"加急 · "+STATUS_LABELS[task.status]:STATUS_LABELS[task.status],alert?1011:1031,100);
  context.textAlign="left";

  context.font="800 168px Consolas,'Cascadia Mono',monospace";context.fillStyle=COLORS.blue;
  context.fillText(displayTicket(task),90,310);
  context.fillStyle=COLORS.amber;context.fillRect(92,410,240,12);

  context.font="700 46px 'Microsoft YaHei UI','Microsoft YaHei',sans-serif";context.fillStyle=COLORS.ink;
  fitLines(context,task.title,980,2).forEach((line,index)=>context.fillText(line,90,502+index*62));

  const y=650;context.fillStyle="#E8EDF5";context.fillRect(90,y-36,1020,1);
  const fields=[["部门 / 团队",task.department],["对接人",task.contact],["事项类型",task.taskType],["截止时间",dateTime(task.requestedDeadline)]];
  fields.forEach(([label,value],index)=>{
    const x=90+index*255;
    context.font="500 17px 'Microsoft YaHei UI','Microsoft YaHei',sans-serif";context.fillStyle=COLORS.muted;context.fillText(label,x,y);
    context.font="600 23px 'Microsoft YaHei UI','Microsoft YaHei',sans-serif";context.fillStyle=COLORS.ink;
    const short=value.length>11?value.slice(0,10)+"…":value;context.fillText(short,x,y+38);
  });
  context.font="500 16px Consolas,monospace";context.fillStyle="#7B8797";context.fillText(task.permanentNumber,90,738);
  context.textAlign="right";context.fillText("取号只记录登记顺序",1110,738);
  const blob=await new Promise<Blob>((resolve,reject)=>canvas.toBlob(value=>value?resolve(value):reject(new Error("取号图片生成失败")),"image/png"));
  return new Uint8Array(await blob.arrayBuffer());
}
