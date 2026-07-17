import type { LegalTask } from "../types";
import { displayTicket,formatDateTime,STATUS_LABELS } from "./task-utils";

const WIDTH=800,HEIGHT=1200;
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
export interface TicketRgbaImage{rgba:Uint8Array;width:number;height:number}

export async function renderTicketRgba(task:LegalTask):Promise<TicketRgbaImage>{
  await document.fonts.ready;
  const canvas=document.createElement("canvas");canvas.width=WIDTH;canvas.height=HEIGHT;
  const context=canvas.getContext("2d");if(!context)throw new Error("当前设备无法生成取号图片");
  context.textBaseline="middle";context.fillStyle=COLORS.paper;context.fillRect(0,0,WIDTH,HEIGHT);
  rounded(context,30,30,740,1140,34,"#FFFFFF");
  context.fillStyle=COLORS.blue;context.fillRect(30,30,14,1140);
  context.font="700 28px 'Microsoft YaHei UI','Microsoft YaHei',sans-serif";context.fillStyle=COLORS.ink;context.fillText("IN LINE",72,88);
  context.font="500 19px 'Microsoft YaHei UI','Microsoft YaHei',sans-serif";context.fillStyle=COLORS.muted;context.fillText("事项已登记，请按队列处理",72,128);
  const alert=task.isUrgent||task.priority==="critical";
  rounded(context,alert?574:614,68,alert?154:114,48,24,alert?COLORS.red:COLORS.blue);
  context.font="700 19px 'Microsoft YaHei UI','Microsoft YaHei',sans-serif";context.fillStyle="#FFF";context.textAlign="center";
  context.fillText(alert?"加急 · "+STATUS_LABELS[task.status]:STATUS_LABELS[task.status],alert?651:671,92);
  context.textAlign="left";

  context.font="800 158px Consolas,'Cascadia Mono',monospace";context.fillStyle=COLORS.blue;
  context.textAlign="center";
  context.fillText(displayTicket(task),400,306);
  context.fillStyle=COLORS.amber;context.fillRect(280,404,240,12);

  context.font="700 43px 'Microsoft YaHei UI','Microsoft YaHei',sans-serif";context.fillStyle=COLORS.ink;
  context.textAlign="left";
  fitLines(context,task.title,656,2).forEach((line,index)=>context.fillText(line,72,510+index*60));

  context.fillStyle="#E8EDF5";context.fillRect(72,650,656,1);
  const fields=[["部门 / 团队",task.department],["对接人",task.contact],["事项类型",task.taskType],["截止时间",formatDateTime(task.requestedDeadline)]];
  fields.forEach(([label,value],index)=>{
    const x=72+(index%2)*344;
    const y=724+Math.floor(index/2)*154;
    context.font="500 18px 'Microsoft YaHei UI','Microsoft YaHei',sans-serif";context.fillStyle=COLORS.muted;context.fillText(label,x,y);
    context.font="600 25px 'Microsoft YaHei UI','Microsoft YaHei',sans-serif";context.fillStyle=COLORS.ink;
    fitLines(context,value,286,2).forEach((line,lineIndex)=>context.fillText(line,x,y+42+lineIndex*34));
  });
  context.font="500 16px Consolas,monospace";context.fillStyle="#7B8797";context.fillText(task.permanentNumber,72,1124);
  context.textAlign="right";
  context.fillText("取号只记录登记顺序",728,1124);
  const rgba=context.getImageData(0,0,WIDTH,HEIGHT).data;
  return{rgba:new Uint8Array(rgba),width:WIDTH,height:HEIGHT};
}
