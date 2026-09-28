import { useMemo, useState, type FormEvent } from "react";
import { AlertTriangle, Check, ChevronDown, ChevronUp, CornerDownRight, X } from "lucide-react";
import type { CreateSubtaskInput, LegalTask, MasterData, Priority, Workload } from "../types";
import { api } from "../api";
import { createSubtaskDraft } from "../lib/subtask-ui";
import ComboInput from "./ComboInput";
import DeadlinePicker from "./DeadlinePicker";
import MultiContactInput from "./MultiContactInput";
import StatusBadge from "./StatusBadge";
import TicketNumber from "./TicketNumber";

interface Props {
  parent: LegalTask;
  masters: MasterData;
  commonDepartments: string[];
  commonContacts: string[];
  onClose: () => void;
  onSaved: (task: LegalTask) => void;
}

export default function SubtaskCreateDialog({ parent, masters, commonDepartments, commonContacts, onClose, onSaved }: Props) {
  const [form, setForm] = useState<CreateSubtaskInput>(() => createSubtaskDraft(parent));
  const [expanded, setExpanded] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const quickDepartments = useMemo(() => [...new Set(commonDepartments)].slice(0, 3), [commonDepartments]);
  const quickContacts = useMemo(() => [...new Set(commonContacts)].slice(0, 3), [commonContacts]);

  const update = <K extends keyof CreateSubtaskInput>(key: K, value: CreateSubtaskInput[K]) => {
    setForm(current => ({ ...current, [key]: value }));
  };
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setSaving(true);
    setError("");
    try {
      onSaved(await api.createSubtask(form));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSaving(false);
    }
  };

  return <div className="modal-layer subtask-create-layer" role="presentation" onMouseDown={event => event.target === event.currentTarget && onClose()}>
    <section className="task-form-panel subtask-create-panel" role="dialog" aria-modal="true" aria-labelledby="subtask-create-title">
      <header className="form-header">
        <div><span className="form-kicker">轻量创建</span><h2 id="subtask-create-title">添加子任务</h2></div>
        <button type="button" className="icon-button" onClick={onClose} aria-label="关闭"><X size={18} /></button>
      </header>
      <form onSubmit={submit}>
        {error && <div className="form-error"><AlertTriangle size={15} />{error}</div>}
        <div className="subtask-parent-summary" aria-label="所属父任务">
          <CornerDownRight size={18} />
          <div><span>所属任务已确定</span><strong>{parent.title}</strong></div>
          <TicketNumber task={parent} />
          <StatusBadge status={parent.status} />
        </div>
        <label className="subtask-title-field">
          <span>子任务名称 *</span>
          <input autoFocus required maxLength={100} value={form.title} onChange={event => update("title", event.target.value)} placeholder="一句话说明要处理的子任务" />
        </label>
        <label className="subtask-enqueue-switch">
          <span><strong>立即加入今日队列</strong><small>开启后，子任务会独立取得今日编号</small></span>
          <span className="switch"><input type="checkbox" checked={form.enqueueToday} onChange={event => update("enqueueToday", event.target.checked)} /><span /></span>
        </label>
        <button type="button" className="subtask-more-toggle" aria-expanded={expanded} onClick={() => setExpanded(current => !current)}>
          {expanded ? <ChevronUp size={16} /> : <ChevronDown size={16} />}更多设置
          <small>{expanded ? "收起非必填内容" : "截止时间、加急、详情及其他属性"}</small>
        </button>
        {expanded && <div className="form-grid subtask-more-fields">
          <label className="span-2"><span>事件详情</span><textarea rows={3} value={form.details} onChange={event => update("details", event.target.value)} placeholder="补充背景、具体要求、关键时间点或现有材料" /></label>
          <label className="paired-control-field"><span>部门 / 团队 *</span><MultiContactInput values={form.departments ?? []} options={masters.departments} commonOptions={quickDepartments} itemLabel="部门 / 团队" onChange={departments => update("departments", departments)} /></label>
          <label className="paired-control-field"><span>对接人 *</span><MultiContactInput values={form.contacts ?? []} options={masters.contacts} commonOptions={quickContacts} onChange={contacts => update("contacts", contacts)} /></label>
          <label className="paired-control-field"><span>事项类型 *</span><ComboInput value={form.taskType ?? ""} options={masters.taskTypes} onChange={taskType => update("taskType", taskType)} placeholder="输入或选择事项类型" /></label>
          <label className="paired-control-field"><span>要求完成时间</span><DeadlinePicker value={form.requestedDeadline ?? null} label={form.requestedDeadlineLabel ?? null} onChange={(value, label) => setForm(current => ({ ...current, requestedDeadline: value, requestedDeadlineLabel: label }))} /></label>
          <label><span>优先级</span><select value={form.priority} onChange={event => update("priority", event.target.value as Priority)}><option value="normal">普通</option><option value="elevated">较急</option><option value="urgent">紧急</option><option value="critical">重大紧急</option></select></label>
          <label><span>预计工作量</span><select value={form.workload} onChange={event => update("workload", event.target.value as Workload)}><option value="simple">简单</option><option value="standard">一般</option><option value="complex">复杂</option><option value="major">重大</option></select></label>
          <label className="urgent-check span-2"><input type="checkbox" checked={form.isUrgent} onChange={event => update("isUrgent", event.target.checked)} /><span>标记为加急事项（不继承父任务）</span></label>
          {form.isUrgent && <div className="urgent-fields span-2"><label><span>加急申请人 *</span><input required value={form.urgentRequester} onChange={event => update("urgentRequester", event.target.value)} /></label><label><span>加急原因 *</span><input required value={form.urgentReason} onChange={event => update("urgentReason", event.target.value)} /></label></div>}
          <label className="span-2"><span>内部备注</span><textarea rows={2} value={form.internalNotes} onChange={event => update("internalNotes", event.target.value)} placeholder="记录判断、风险或后续计划，仅保存在本机" /></label>
        </div>}
        <footer className="form-actions">
          <span className="keyboard-hint">部门、对接人和事项类型已从父任务复制，创建后可独立修改</span>
          <div><button type="button" className="button secondary" onClick={onClose}>取消</button><button className="button primary" disabled={saving || !form.title.trim()}><Check size={16} />{saving ? "创建中" : form.enqueueToday ? "创建并取号" : "仅创建"}</button></div>
        </footer>
      </form>
    </section>
  </div>;
}
