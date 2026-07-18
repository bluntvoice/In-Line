import { Check, Filter, RotateCcw } from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { DEADLINE_PERIOD_LABELS, type DeadlinePeriod, type ValueSelection } from "../lib/task-filters";

interface ShellProps {
  label: string;
  active: boolean;
  children: (close: () => void) => ReactNode;
}

function FilterShell({ label, active, children }: ShellProps) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("pointerdown", close);
      document.removeEventListener("keydown", escape);
    };
  }, [open]);

  return <div className="table-filter" ref={root}>
    <button type="button" className={active ? "table-filter-trigger active" : "table-filter-trigger"} aria-haspopup="dialog" aria-expanded={open} onClick={() => setOpen((value) => !value)}>
      <span>{label}</span><Filter size={13} aria-hidden="true" />
    </button>
    {open && <div className="table-filter-popover" role="dialog" aria-label={`${label}筛选`} onClick={(event) => event.stopPropagation()}>
      {children(() => setOpen(false))}
    </div>}
  </div>;
}

interface ValueProps<T extends string> {
  label: string;
  values: T[];
  selected: ValueSelection<T>;
  renderLabel?: (value: T) => string;
  onChange: (value: ValueSelection<T>) => void;
}

export function ValueFilterHeader<T extends string>({ label, values, selected, renderLabel = (value) => value, onChange }: ValueProps<T>) {
  const toggle = (value: T) => {
    const current = selected === null ? values : selected;
    const next = current.includes(value) ? current.filter((item) => item !== value) : [...current, value];
    onChange(next.length === values.length ? null : next);
  };

  return <FilterShell label={label} active={selected !== null}>{(close) => <>
    <div className="filter-popover-heading"><strong>{label}</strong><button type="button" onClick={() => onChange(null)}><RotateCcw size={13} />重置</button></div>
    <div className="filter-shortcuts"><button type="button" onClick={() => onChange(null)}>全选</button><button type="button" onClick={() => onChange([])}>清空</button></div>
    <div className="filter-value-list">
      {values.map((value) => {
        const checked = selected === null || selected.includes(value);
        return <label key={value}><input type="checkbox" checked={checked} onChange={() => toggle(value)} /><span className="filter-check">{checked && <Check size={12} />}</span><span>{renderLabel(value)}</span></label>;
      })}
      {!values.length && <p>当前页面暂无可筛选内容</p>}
    </div>
    <button type="button" className="filter-done" onClick={close}>完成</button>
  </>}</FilterShell>;
}

interface DeadlineProps {
  date: string;
  periods: DeadlinePeriod[];
  onChange: (date: string, periods: DeadlinePeriod[]) => void;
}

const periods = Object.keys(DEADLINE_PERIOD_LABELS) as DeadlinePeriod[];

export function DeadlineFilterHeader({ date, periods: selected, onChange }: DeadlineProps) {
  const toggle = (period: DeadlinePeriod) => onChange(date, selected.includes(period) ? selected.filter((value) => value !== period) : [...selected, period]);
  return <FilterShell label="截止时间" active={Boolean(date || selected.length)}>{(close) => <>
    <div className="filter-popover-heading"><strong>截止时间</strong><button type="button" onClick={() => onChange("", [])}><RotateCcw size={13} />重置</button></div>
    <label className="deadline-filter-date"><span>按天筛选</span><input type="date" value={date} onChange={(event) => onChange(event.target.value, selected)} /></label>
    <fieldset className="deadline-periods"><legend>按时段筛选</legend>{periods.map((period) => {
      const checked = selected.includes(period);
      return <label key={period}><input type="checkbox" checked={checked} onChange={() => toggle(period)} /><span className="filter-check">{checked && <Check size={12} />}</span><span>{DEADLINE_PERIOD_LABELS[period]}</span></label>;
    })}</fieldset>
    <p className="filter-note">上午 05–11 时；中午 11–14 时；下午 14–18 时；其余归入晚上。</p>
    <button type="button" className="filter-done" onClick={close}>完成</button>
  </>}</FilterShell>;
}
