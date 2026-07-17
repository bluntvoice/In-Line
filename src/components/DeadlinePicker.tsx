import { Check, Clock3, X } from "lucide-react";
import { useEffect, useState, type KeyboardEvent } from "react";
import { deadlineShortcut, formatDeadline, fromDateTimeLocalValue, toDateTimeLocalValue, type DeadlineShortcut } from "../lib/task-utils";

interface Props {
  value: string | null;
  label: string | null;
  onChange: (value: string | null, label: string | null) => void;
}

const QUICK_OPTIONS: Array<{ kind: DeadlineShortcut; text: string }> = [
  { kind: "half_hour", text: "半小时后" },
  { kind: "one_hour", text: "1 小时后" },
  { kind: "morning", text: "上午" },
  { kind: "noon", text: "中午" },
  { kind: "afternoon", text: "下午" },
  { kind: "before_off_work", text: "下班前" }
];

export default function DeadlinePicker({ value, label, onChange }: Props) {
  const [draft, setDraft] = useState(() => toDateTimeLocalValue(value));
  useEffect(() => setDraft(toDateTimeLocalValue(value)), [value]);

  const confirm = () => {
    const next = fromDateTimeLocalValue(draft);
    if (next) onChange(next, null);
  };
  const confirmFromKeyboard = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== "Enter") return;
    event.preventDefault();
    confirm();
  };
  const useQuick = (kind: DeadlineShortcut) => {
    const next = deadlineShortcut(kind);
    setDraft(toDateTimeLocalValue(next.value));
    onChange(next.value, next.label);
  };

  return <div className="deadline-picker">
    <div className="deadline-input-row">
      <div className="deadline-input"><Clock3 size={16} /><input type="datetime-local" value={draft} onChange={(event) => setDraft(event.target.value)} onKeyDown={confirmFromKeyboard} /></div>
      <button type="button" className="deadline-confirm" disabled={!draft} onClick={confirm}><Check size={15} />确认</button>
      {(value || draft) && <button type="button" className="deadline-clear" title="清除要求完成时间" aria-label="清除要求完成时间" onClick={() => { setDraft(""); onChange(null, null); }}><X size={15} /></button>}
    </div>
    <div className="deadline-quick" aria-label="快捷时间选项">
      {QUICK_OPTIONS.map((item) => <button type="button" key={item.kind} onClick={() => useQuick(item.kind)}>{item.text}</button>)}
    </div>
    {value && <span className="deadline-current">已设置：{formatDeadline(value, label)}</span>}
  </div>;
}
