import type { StatisticsResult } from "../types";
import type { StatisticsChartId,TaskTypeChartMode } from "./statistics-charts";

export const STATISTICS_EXPORT_DPI=300;
const CSS_DPI=96;
const PNG_SIGNATURE=[137,80,78,71,13,10,26,10];
const COLORS={ink:"#12243a",muted:"#61748a",line:"#dbe4ee",paper:"#ffffff",page:"#eef3f8",blue:"#0b3a82",blueSoft:"#73a7d8",green:"#2b8a68",red:"#c95245"};
const PIE_COLORS=["#0b3a82","#2f69a8","#55a0a6","#78a85a","#c68a16","#ce6b54","#7d68a8","#7890aa"];

export interface StatisticsExportData {
  data:StatisticsResult;
  comparisonData:StatisticsResult;
  range:{start:string;end:string};
  comparisonRange:{start:string;end:string};
  comparisonLabel:string;
  taskTypeMode:TaskTypeChartMode;
}

function uint32(value:number){return new Uint8Array([(value>>>24)&255,(value>>>16)&255,(value>>>8)&255,value&255]);}
function crc32(bytes:Uint8Array){let crc=0xffffffff;for(const byte of bytes){crc^=byte;for(let bit=0;bit<8;bit++)crc=(crc>>>1)^((crc&1)?0xedb88320:0);}return (crc^0xffffffff)>>>0;}
function pngChunk(type:string,data:Uint8Array){const typeBytes=new TextEncoder().encode(type);const body=new Uint8Array(typeBytes.length+data.length);body.set(typeBytes);body.set(data,typeBytes.length);const chunk=new Uint8Array(12+data.length);chunk.set(uint32(data.length));chunk.set(body,4);chunk.set(uint32(crc32(body)),8+data.length);return chunk;}

export function setPngDpi(bytes:Uint8Array,dpi=STATISTICS_EXPORT_DPI){
  if(bytes.length<33||PNG_SIGNATURE.some((value,index)=>bytes[index]!==value))throw new Error("生成的图表不是有效 PNG 文件");
  const pixelsPerMeter=Math.round(dpi/0.0254);const data=new Uint8Array(9);data.set(uint32(pixelsPerMeter),0);data.set(uint32(pixelsPerMeter),4);data[8]=1;
  const chunk=pngChunk("pHYs",data);const output=new Uint8Array(bytes.length+chunk.length);output.set(bytes.slice(0,33));output.set(chunk,33);output.set(bytes.slice(33),33+chunk.length);return output;
}

function roundedRect(context:CanvasRenderingContext2D,x:number,y:number,width:number,height:number,radius=14){
  const r=Math.min(radius,width/2,height/2);context.beginPath();context.moveTo(x+r,y);context.arcTo(x+width,y,x+width,y+height,r);context.arcTo(x+width,y+height,x,y+height,r);context.arcTo(x,y+height,x,y,r);context.arcTo(x,y,x+width,y,r);context.closePath();
}
function fillText(context:CanvasRenderingContext2D,text:string,x:number,y:number,size=14,color=COLORS.ink,weight=400){context.font=`${weight} ${size}px "Microsoft YaHei","Segoe UI",sans-serif`;context.fillStyle=color;context.fillText(text,x,y);}
function ellipsis(context:CanvasRenderingContext2D,text:string,maxWidth:number){if(context.measureText(text).width<=maxWidth)return text;let value=text;while(value.length>1&&context.measureText(`${value}…`).width>maxWidth)value=value.slice(0,-1);return `${value}…`;}
function cardFrame(context:CanvasRenderingContext2D,x:number,y:number,width:number,height:number,title:string,subtitle:string){context.fillStyle=COLORS.paper;roundedRect(context,x,y,width,height,18);context.fill();context.strokeStyle=COLORS.line;context.lineWidth=1;context.stroke();fillText(context,title,x+24,y+36,19,COLORS.ink,700);fillText(context,subtitle,x+24,y+61,12,COLORS.muted,400);context.strokeStyle=COLORS.line;context.beginPath();context.moveTo(x+24,y+78);context.lineTo(x+width-24,y+78);context.stroke();}
function delta(current:number,previous:number){if(previous===0)return current===0?"持平":"新增";const value=Math.round((current-previous)/previous*100);return `${value>=0?"+":""}${value}%`;}

export function trendBarLayout(chartWidth:number,count:number,index:number){
  const safeCount=Math.max(1,count),slotWidth=chartWidth/safeCount,barWidth=Math.min(22,Math.max(8,slotWidth*.44));
  return{barWidth,barX:index*slotWidth+(slotWidth-barWidth)/2,labelX:index*slotWidth+slotWidth/2};
}

export function pieVerticalLayout(y:number,height:number,rowCount:number){
  const contentTop=y+88,contentBottom=y+height-28,centerY=(contentTop+contentBottom)/2,legendHeight=Math.max(35,rowCount*35);
  return{centerY,legendTop:centerY-legendHeight/2,legendRowHeight:35};
}

function workloadRows(data:StatisticsResult,comparison:StatisticsResult){return[
  {label:"处理事项",current:data.summary.handledTasks,previous:comparison.summary.handledTasks},
  {label:"已完成",current:data.summary.completed,previous:comparison.summary.completed},
  {label:"待跟进",current:data.summary.handledTasks-data.summary.completed,previous:comparison.summary.handledTasks-comparison.summary.completed}
];}
function departmentRows(data:StatisticsResult,comparison:StatisticsResult){const names=new Set([...data.byDepartment.map(item=>item.department),...comparison.byDepartment.map(item=>item.department)]);return [...names].map(department=>({label:department,current:data.byDepartment.find(item=>item.department===department)?.handledTasks??0,previous:comparison.byDepartment.find(item=>item.department===department)?.handledTasks??0})).sort((a,b)=>Math.max(b.current,b.previous)-Math.max(a.current,a.previous)||a.label.localeCompare(b.label,"zh-CN"));}

function drawComparisonRows(context:CanvasRenderingContext2D,x:number,y:number,width:number,rows:Array<{label:string;current:number;previous:number}>,comparisonLabel:string){
  const max=Math.max(1,...rows.flatMap(row=>[row.current,row.previous]));
  rows.forEach((row,index)=>{const top=y+index*56;fillText(context,ellipsis(context,row.label,118),x,top+13,13,COLORS.ink,600);const tone=row.current>row.previous?COLORS.red:row.current<row.previous?COLORS.green:COLORS.muted;fillText(context,delta(row.current,row.previous),x+120,top+13,11,tone,600);const barX=x+175,barWidth=width-210;context.fillStyle="#e4ebf3";roundedRect(context,barX,top,barWidth,10,5);context.fill();if(row.current){context.fillStyle=COLORS.blue;roundedRect(context,barX,top,Math.max(4,barWidth*row.current/max),10,5);context.fill();}fillText(context,String(row.current),barX+barWidth+8,top+9,11,tone,700);context.fillStyle="#e4ebf3";roundedRect(context,barX,top+20,barWidth,8,4);context.fill();if(row.previous){context.fillStyle=COLORS.blueSoft;roundedRect(context,barX,top+20,Math.max(4,barWidth*row.previous/max),8,4);context.fill();}fillText(context,String(row.previous),barX+barWidth+8,top+27,10,COLORS.muted,600);});
  fillText(context,`深蓝：当前周期    浅蓝：${comparisonLabel}`,x,y+rows.length*56+5,11,COLORS.muted,400);
}

function drawWorkload(context:CanvasRenderingContext2D,x:number,y:number,width:number,height:number,options:StatisticsExportData){cardFrame(context,x,y,width,height,"周期工作量对比",`当前周期与${options.comparisonLabel} · 仅统计数量`);drawComparisonRows(context,x+24,y+103,width-48,workloadRows(options.data,options.comparisonData),options.comparisonLabel);fillText(context,`对比范围：${options.comparisonRange.start} 至 ${options.comparisonRange.end}`,x+24,y+height-20,11,COLORS.muted);}
function drawDepartment(context:CanvasRenderingContext2D,x:number,y:number,width:number,height:number,options:StatisticsExportData){const rows=departmentRows(options.data,options.comparisonData);cardFrame(context,x,y,width,height,"部门 / 团队相关工作量","按事项当前归属拆分 · 仅统计数量");if(rows.length)drawComparisonRows(context,x+24,y+103,width-48,rows,options.comparisonLabel);else fillText(context,"当前及对比周期均无部门相关工作量。",x+24,y+122,13,COLORS.muted);fillText(context,"同一事项归属多个部门时会分别计入。",x+24,y+height-20,11,COLORS.muted);}
function drawTaskTypes(context:CanvasRenderingContext2D,x:number,y:number,width:number,height:number,options:StatisticsExportData){const rows=options.data.byTaskType;cardFrame(context,x,y,width,height,"事项类型分布",options.taskTypeMode==="pie"?"当前周期 · 饼状分布":"当前周期 · 按处理数量排列");if(!rows.length){fillText(context,"暂无事项类型数据。",x+24,y+122,13,COLORS.muted);return;}const total=Math.max(1,rows.reduce((sum,item)=>sum+item.handledTasks,0));if(options.taskTypeMode==="pie"){const centerX=x+119,{centerY,legendTop,legendRowHeight}=pieVerticalLayout(y,height,rows.length),radius=90;let angle=-Math.PI/2;rows.forEach((item,index)=>{const next=angle+Math.PI*2*item.handledTasks/total;context.beginPath();context.moveTo(centerX,centerY);context.arc(centerX,centerY,radius,angle,next);context.closePath();context.fillStyle=PIE_COLORS[index%PIE_COLORS.length];context.fill();angle=next;});context.fillStyle=COLORS.paper;context.beginPath();context.arc(centerX,centerY,54,0,Math.PI*2);context.fill();context.textAlign="center";fillText(context,String(total),centerX,centerY+4,24,COLORS.ink,700);fillText(context,"处理事项",centerX,centerY+25,11,COLORS.muted,400);context.textAlign="left";rows.forEach((item,index)=>{const top=legendTop+index*legendRowHeight,legendX=x+232;context.fillStyle=PIE_COLORS[index%PIE_COLORS.length];roundedRect(context,legendX,top,10,10,3);context.fill();fillText(context,ellipsis(context,item.taskType,112),legendX+18,top+10,12,COLORS.ink,600);fillText(context,`${item.handledTasks} 项 · ${Math.round(item.handledTasks/total*100)}%`,x+width-126,top+10,11,COLORS.muted);});}else{rows.forEach((item,index)=>{const top=y+102+index*42;fillText(context,ellipsis(context,item.taskType,width-180),x+24,top+14,13,COLORS.ink,600);fillText(context,`完成 ${item.completed} · 待跟进 ${item.pendingFollowUp}`,x+24,top+33,11,COLORS.muted);fillText(context,`${item.handledTasks} 项`,x+width-76,top+22,13,COLORS.blue,700);if(index<rows.length-1){context.strokeStyle=COLORS.line;context.beginPath();context.moveTo(x+24,top+41);context.lineTo(x+width-24,top+41);context.stroke();}});}}
function drawTrend(context:CanvasRenderingContext2D,x:number,y:number,width:number,height:number,options:StatisticsExportData){const rows=options.data.trend;cardFrame(context,x,y,width,height,"工作量趋势",options.data.trendGranularity==="day"?"按日去重":"按自然周去重");if(!rows.length){fillText(context,"暂无趋势数据。",x+24,y+122,13,COLORS.muted);return;}const chartX=x+36,chartY=y+105,chartWidth=width-72,chartHeight=height-158,max=Math.max(1,...rows.map(item=>item.handledTasks));context.strokeStyle=COLORS.line;context.beginPath();context.moveTo(chartX,chartY+chartHeight);context.lineTo(chartX+chartWidth,chartY+chartHeight);context.stroke();rows.forEach((item,index)=>{const barHeight=item.handledTasks?Math.max(5,chartHeight*item.handledTasks/max):2,{barWidth,barX,labelX}=trendBarLayout(chartWidth,rows.length,index);context.fillStyle=COLORS.blue;roundedRect(context,chartX+barX,chartY+chartHeight-barHeight,barWidth,barHeight,Math.min(5,barWidth/2));context.fill();context.textAlign="center";fillText(context,String(item.handledTasks),chartX+labelX,chartY+chartHeight-barHeight-7,10,COLORS.ink,600);fillText(context,item.periodStart.slice(5),chartX+labelX,chartY+chartHeight+20,9,COLORS.muted);context.textAlign="left";});}

function cardHeight(id:StatisticsChartId,options:StatisticsExportData){if(id==="workload")return 330;if(id==="department")return Math.max(310,155+departmentRows(options.data,options.comparisonData).length*56);if(id==="taskType")return options.taskTypeMode==="pie"?Math.max(350,145+options.data.byTaskType.length*31):Math.max(310,135+options.data.byTaskType.length*42);return 330;}

export async function renderStatisticsChartsPng(ids:StatisticsChartId[],title:string,options:StatisticsExportData){
  if(!ids.length)throw new Error("请至少选择一张图表");
  const pageWidth=1120,padding=40,gap=24,cardWidth=(pageWidth-padding*2-gap)/2,rows:Array<{ids:StatisticsChartId[];height:number}>=[];
  for(let index=0;index<ids.length;index+=2){const pair=ids.slice(index,index+2);rows.push({ids:pair,height:Math.max(...pair.map(id=>cardHeight(id,options)))});}
  const headerHeight=104,pageHeight=padding+headerHeight+rows.reduce((sum,row)=>sum+row.height,0)+gap*Math.max(0,rows.length-1)+padding;
  const scale=STATISTICS_EXPORT_DPI/CSS_DPI,canvas=document.createElement("canvas");canvas.width=Math.ceil(pageWidth*scale);canvas.height=Math.ceil(pageHeight*scale);const context=canvas.getContext("2d");if(!context)throw new Error("当前设备无法生成图表图片");context.scale(scale,scale);context.fillStyle=COLORS.page;context.fillRect(0,0,pageWidth,pageHeight);
  fillText(context,"IN LINE · 排着呢",padding,padding+18,12,COLORS.blue,700);fillText(context,title,padding,padding+58,30,COLORS.ink,700);fillText(context,`${options.range.start} 至 ${options.range.end}`,pageWidth-padding-205,padding+58,13,COLORS.muted,500);
  let y=padding+headerHeight;rows.forEach(row=>{row.ids.forEach((id,index)=>{const x=padding+index*(cardWidth+gap);if(id==="workload")drawWorkload(context,x,y,cardWidth,row.height,options);else if(id==="department")drawDepartment(context,x,y,cardWidth,row.height,options);else if(id==="taskType")drawTaskTypes(context,x,y,cardWidth,row.height,options);else drawTrend(context,x,y,cardWidth,row.height,options);});y+=row.height+gap;});
  const blob=await new Promise<Blob>((resolve,reject)=>canvas.toBlob(value=>value?resolve(value):reject(new Error("图表图片编码失败")),"image/png"));return setPngDpi(new Uint8Array(await blob.arrayBuffer()));
}
