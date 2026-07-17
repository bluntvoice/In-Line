import { useEffect, useMemo, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { AlertTriangle, Check, X } from "lucide-react";
import type { LegalTask, MasterData, Priority, TaskInput, TaskStatus, Workload } from "../types";
import { api } from "../api";

interface Props {
  task: LegalTask | null;
  masters: MasterData;
  recentContacts: string[];
  onClose: () => void;
  onSaved: () => void;
}

const emptyTask: TaskInput = {
  department: "",
  contact: "",
  taskType: "任务处理",
  title: "",
  details: "",
  status: "pending",
  priority: "normal",
  workload: "standard",
  isUrgent: false,
  urgentRequester: "",
  urgentReason: "",
  requestedDeadline: null,
  internalNotes: ""
};

function toInput(task: LegalTask | null): TaskInput {
  if (!task) return { ...emptyTask };
  return {
    id: task.id,
    department: task.department,
    contact: task.contact,
    taskType: task.taskType,
    title: task.title,
    details: task.details,
    status: task.status,
    priority: task.priority,
    workload: task.workload,
    isUrgent: task.isUrgent,
    urgentRequester: task.urgentRequester,
    urgentReason: task.urgentReason,
    requestedDeadline: task.requestedDeadline,
    internalNotes: task.internalNotes
  };
}

export default function TaskForm({ task, masters, recentContacts, onClose, onSaved }: Props) {
  const [form, setForm] = useState<TaskInput>(() => toInput(task));
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const firstInput = useRef<HTMLInputElement>(null);

  useEffect(() => firstInput.current?.focus(), []);
  const title = task ? `编辑 ${task.permanentNumber}` : "新增取号";
  const uniqueContacts = useMemo(() => [...new Set(recentContacts)].slice(0, 5), [recentContacts]);

  const update = <K extends keyof TaskInput>(key: K, value: TaskInput[K]) => {
    setForm((current) => ({ ...current, [key]: value }));
  };

  const submit = async (event?: FormEvent) => {
    event?.preventDefault();
    setSaving(true);
    setError("");
    try {
      await api.saveTask(form);
      onSaved();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSaving(false);
    }
  };

  const enterToSave = (event: KeyboardEvent<HTMLFormElement>) => {
    if (event.key !== "Enter" || event.shiftKey) return;
    const target = event.target as HTMLElement;
    if (target.tagName === "TEXTAREA" || target.tagName === "BUTTON") return;
    event.preventDefault();
    void submit();
  };

  return (
    <div className="modal-layer" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <section className="task-form-panel" role="dialog" aria-modal="true" aria-labelledby="task-form-title">
        <header className="form-header">
          <div>
            <span className="form-kicker">事项登记</span>
            <h2 id="task-form-title">{title}</h2>
          </div>
          <button className="icon-button" onClick={onClose} aria-label="关闭"><X size={18} /></button>
        </header>

        <form onSubmit={submit} onKeyDown={enterToSave}>
          {error && <div className="form-error"><AlertTriangle size={15} />{error}</div>}
          <div className="form-grid">
            <label>
              <span>部门 / 团队 *</span>
              <input ref={firstInput} list="departments" value={form.department} onChange={(e) => update("department", e.target.value)} placeholder="例如：产品组" />
              <datalist id="departments">{masters.departments.map((item) => <option key={item}>{item}</option>)}</datalist>
            </label>
            <label>
              <span>对接人 *</span>
              <input value={form.contact} onChange={(e) => update("contact", e.target.value)} placeholder="姓名" />
              {uniqueContacts.length > 0 && (
                <span className="recent-contacts">
                  {uniqueContacts.map((name) => <button type="button" key={name} onClick={() => update("contact", name)}>{name}</button>)}
                </span>
              )}
            </label>
            <label>
              <span>事项类型 *</span>
              <input list="task-types" value={form.taskType} onChange={(e) => update("taskType", e.target.value)} />
              <datalist id="task-types">{masters.taskTypes.map((item) => <option key={item}>{item}</option>)}</datalist>
            </label>
            <label>
              <span>要求完成时间</span>
              <input type="datetime-local" value={form.requestedDeadline?.slice(0, 16) ?? ""} onChange={(e) => update("requestedDeadline", e.target.value ? new Date(e.target.value).toISOString() : null)} />
            </label>
            <label className="span-2">
              <span>事项标题 *</span>
              <input maxLength={100} value={form.title} onChange={(e) => update("title", e.target.value)} placeholder="一句话说明要处理的事项" />
            </label>
            <label className="span-2">
              <span>事件详情 *</span>
              <textarea rows={4} value={form.details} onChange={(e) => update("details", e.target.value)} placeholder="填写背景、具体要求、关键时间点及现有材料" />
            </label>
            <label>
              <span>当前状态</span>
              <select value={form.status} onChange={(e) => update("status", e.target.value as TaskStatus)}>
                <option value="pending">待处理</option><option value="processing">处理中</option>
                <option value="waiting_materials">待补充材料</option><option value="waiting_confirmation">待内部确认</option>
                <option value="paused">已暂停</option><option value="completed">已完成</option><option value="cancelled">已取消</option>
              </select>
            </label>
            <label>
              <span>优先级</span>
              <select value={form.priority} onChange={(e) => update("priority", e.target.value as Priority)}>
                <option value="normal">普通</option><option value="elevated">较急</option><option value="urgent">紧急</option><option value="critical">重大紧急</option>
              </select>
            </label>
            <label>
              <span>预计工作量</span>
              <select value={form.workload} onChange={(e) => update("workload", e.target.value as Workload)}>
                <option value="simple">简单</option><option value="standard">一般</option><option value="complex">复杂</option><option value="major">重大</option>
              </select>
            </label>
            <label className="urgent-check">
              <input type="checkbox" checked={form.isUrgent} onChange={(e) => update("isUrgent", e.target.checked)} />
              <span>标记为加急事项</span>
            </label>
            {form.isUrgent && (
              <div className="urgent-fields span-2">
                <label><span>加急申请人 *</span><input value={form.urgentRequester} onChange={(e) => update("urgentRequester", e.target.value)} /></label>
                <label><span>加急原因 *</span><input value={form.urgentReason} onChange={(e) => update("urgentReason", e.target.value)} /></label>
              </div>
            )}
            <label className="span-2">
              <span>内部备注</span>
              <textarea rows={2} value={form.internalNotes} onChange={(e) => update("internalNotes", e.target.value)} placeholder="记录判断、风险或后续计划，仅保存在本机" />
            </label>
          </div>
          <footer className="form-actions">
            <span className="keyboard-hint">Tab 依次填写 · 非多行输入框按 Enter 保存</span>
            <div><button type="button" className="button secondary" onClick={onClose}>取消</button><button className="button primary" disabled={saving}><Check size={16} />{saving ? "保存中" : task ? "保存修改" : "保存并取号"}</button></div>
          </footer>
        </form>
      </section>
    </div>
  );
}
