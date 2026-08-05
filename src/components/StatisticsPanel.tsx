import { useEffect,useMemo,useState } from "react";
import { BarChart3,ChevronRight,RefreshCw } from "lucide-react";
import { api } from "../api";
import type { StatisticsDetail,StatisticsResult } from "../types";
import { formatDateTime,STATUS_LABELS } from "../lib/task-utils";
import { statisticsPresetRange,type StatisticsPreset,type WeekStart } from "../lib/statistics-range";

const localStart=(value:string)=>new Date(`${value}T00:00:00`).toISOString();
const nextLocalDay=(value:string)=>{const date=new Date(`${value}T00:00:00`);date.setDate(date.getDate()+1);return date.toISOString();};

export default function StatisticsPanel({onOpenTask,refreshKey=0,weekStartsOn}:{onOpenTask:(id:number)=>void;refreshKey?:string|number;weekStartsOn:WeekStart}){
  const [preset,setPreset]=useState<StatisticsPreset>("currentWeek");const initial=statisticsPresetRange("currentWeek",weekStartsOn);
  const [customStart,setCustomStart]=useState(initial.start);const [customEnd,setCustomEnd]=useState(initial.end);
  const [data,setData]=useState<StatisticsResult|null>(null);const [loading,setLoading]=useState(true);const [error,setError]=useState("");
  const [selectedType,setSelectedType]=useState("");const [details,setDetails]=useState<StatisticsDetail[]>([]);const [detailsLoading,setDetailsLoading]=useState(false);
  const range=useMemo(()=>preset==="custom"?{start:customStart,end:customEnd}:statisticsPresetRange(preset,weekStartsOn),[preset,customStart,customEnd,weekStartsOn]);
  const load=async()=>{if(!range.start||!range.end||range.start>range.end){setError("开始日期不能晚于结束日期");setLoading(false);return;}setLoading(true);setError("");setSelectedType("");setDetails([]);try{setData(await api.getStatistics(localStart(range.start),nextLocalDay(range.end)));}catch(value){setError(value instanceof Error?value.message:String(value));}finally{setLoading(false);}};
  useEffect(()=>{void load();},[range.start,range.end,refreshKey]);
  const openType=async(taskType:string)=>{setSelectedType(taskType);setDetailsLoading(true);try{setDetails(await api.getStatisticsDetails(localStart(range.start),nextLocalDay(range.end),taskType));}catch(value){setError(value instanceof Error?value.message:String(value));}finally{setDetailsLoading(false);}};
  const maxTrend=Math.max(1,...(data?.trend.map(point=>point.handledTasks)??[1]));
  return <section className="statistics-panel">
    <header className="statistics-header"><div><p>实际处理事项 · 周期内按事项去重</p><h1>统计中心</h1></div><div className="period-controls"><div className="preset-tabs">{(["currentWeek","previousWeek","month","quarter","custom"] as StatisticsPreset[]).map(value=><button className={preset===value?"active":""} key={value} onClick={()=>setPreset(value)}>{value==="currentWeek"?"本周":value==="previousWeek"?"上一周":value==="month"?"上一个月":value==="quarter"?"上一季度":"自定义"}</button>)}</div>{preset==="custom"&&<div className="custom-range"><input type="date" value={customStart} onChange={e=>setCustomStart(e.target.value)}/><span>至</span><input type="date" value={customEnd} onChange={e=>setCustomEnd(e.target.value)}/></div>}<small>{range.start} 至 {range.end}{preset==="currentWeek"&&` · ${weekStartsOn==="monday"?"周一":"周日"}起算`}</small></div></header>
    {error&&<div className="statistics-error"><span>{error}</span><button onClick={()=>void load()}><RefreshCw size={15}/>重新加载</button></div>}
    {loading?<div className="statistics-loading"><BarChart3 size={32}/><p>正在汇总处理活动…</p></div>:data&&<>
      <div className="summary-grid">
        <article><span>处理事项总数</span><strong>{data.summary.handledTasks}</strong></article><article><span>已处理</span><strong>{data.summary.processed}</strong></article><article><span>已完成</span><strong>{data.summary.completed}</strong></article><article><span>待补充材料</span><strong>{data.summary.waitingMaterials}</strong></article><article><span>待内部确认</span><strong>{data.summary.waitingConfirmation}</strong></article><article><span>待对方确认</span><strong>{data.summary.waitingCounterpartyConfirmation}</strong></article><article className="rate-card"><span>完成率</span><strong>{Math.round(data.summary.completionRate*100)}%</strong><small>已完成 ÷ 全部有效处理事项</small></article>
      </div>
      <div className="statistics-grid"><section className="stat-card"><div className="section-heading"><div><h2>事项类型分布</h2><small>点击类型查看当前周期明细</small></div></div>{data.byTaskType.length?<div className="type-stats">{data.byTaskType.map(item=><button key={item.taskType} className={selectedType===item.taskType?"active":""} onClick={()=>void openType(item.taskType)}><span><strong>{item.taskType}</strong><small>已完成 {item.completed} · 待跟进 {item.pendingFollowUp}</small></span><b>{item.handledTasks}</b><ChevronRight size={17}/></button>)}</div>:<p className="empty-copy">当前周期没有有效处理事项。</p>}</section>
        <section className="stat-card trend-card"><div className="section-heading"><div><h2>工作量趋势</h2><small>{data.trendGranularity==="day"?"按日去重":"按自然周去重"}</small></div></div>{data.trend.length?<div className="trend-chart">{data.trend.map(point=><div className="trend-column" key={point.periodStart} title={`${point.periodStart}：${point.handledTasks} 项`}><b>{point.handledTasks}</b><span style={{height:`${Math.max(8,point.handledTasks/maxTrend*140)}px`}}/><small>{point.periodStart.slice(5)}</small></div>)}</div>:<p className="empty-copy">暂无趋势数据。</p>}<p className="trend-note">趋势数据按每日或每周分别去重；同一事项可能在不同日期或不同周重复出现。顶部总数在整个查询周期内统一去重。</p></section>
      </div>
      {selectedType&&<section className="stat-card details-card"><div className="section-heading"><div><h2>{selectedType} · 事项明细</h2><small>同一事项仅显示一行</small></div><b>{details.length} 项</b></div>{detailsLoading?<p className="empty-copy">正在载入明细…</p>:<div className="statistics-table-wrap"><table className="statistics-table"><thead><tr><th>事项编号</th><th>事项标题</th><th>部门 / 团队</th><th>对接人</th><th>最终结果</th><th>首次处理</th><th>最后处理</th><th>次数</th></tr></thead><tbody>{details.map(item=><tr key={item.taskId} onClick={()=>onOpenTask(item.taskId)}><td>{item.permanentNumber}</td><td><strong>{item.title}</strong></td><td>{item.department}</td><td>{item.contact}</td><td>{STATUS_LABELS[item.resultStatus]}</td><td>{formatDateTime(item.firstHandledAt)}</td><td>{formatDateTime(item.lastHandledAt)}</td><td>{item.handlingCount}</td></tr>)}</tbody></table></div>}</section>}
    </>}
  </section>;
}
