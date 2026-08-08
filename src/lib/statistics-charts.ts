export type StatisticsChartId="workload"|"department"|"taskType"|"trend";
export type TaskTypeChartMode="list"|"pie";

export const STATISTICS_CHARTS:{id:StatisticsChartId;label:string;description:string}[]=[
  {id:"workload",label:"周期工作量对比",description:"处理、完成和待跟进事项的周期对比"},
  {id:"department",label:"部门 / 团队相关工作量",description:"按事项当前归属比较部门相关工作量"},
  {id:"taskType",label:"事项类型分布",description:"通过列表或饼图查看类型构成"},
  {id:"trend",label:"工作量趋势",description:"按日或自然周查看处理走势"}
];

export const DEFAULT_STATISTICS_CHART_ORDER=STATISTICS_CHARTS.map(chart=>chart.id);
const chartIds=new Set<StatisticsChartId>(DEFAULT_STATISTICS_CHART_ORDER);

function parsedIds(raw:string|null){
  if(!raw)return[];
  try{
    const value=JSON.parse(raw);
    return Array.isArray(value)?value.filter((id):id is StatisticsChartId=>typeof id==="string"&&chartIds.has(id as StatisticsChartId)):[];
  }catch{return[];}
}

export function normalizeStatisticsChartOrder(raw:string|null){
  const parsed=[...new Set(parsedIds(raw))];
  return [...parsed,...DEFAULT_STATISTICS_CHART_ORDER.filter(id=>!parsed.includes(id))];
}

export function normalizeHiddenStatisticsCharts(raw:string|null){
  return [...new Set(parsedIds(raw))];
}

export function moveStatisticsChart(order:StatisticsChartId[],id:StatisticsChartId,direction:-1|1){
  const current=order.indexOf(id);const target=current+direction;
  if(current<0||target<0||target>=order.length)return order;
  const next=[...order];[next[current],next[target]]=[next[target],next[current]];return next;
}

export function normalizeTaskTypeChartMode(raw:string|null):TaskTypeChartMode{
  return raw==="pie"?"pie":"list";
}
