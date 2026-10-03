import { useEffect, useRef, useState } from "react";
import { Check, Palette, RotateCcw, X } from "lucide-react";
import { DEFAULT_TICKET_COLORS, normalizeHexColor, OFFICIAL_TICKET_COLORS, TICKET_COLOR_LABELS, ticketTextColor, type TicketColorKind } from "../lib/ticket-colors";
import { useTicketColors } from "./TicketColorProvider";

function ColorEditor({ kind, value, busy, resetKey, onSave, onChoose }: {
  kind: TicketColorKind; value: string; busy: boolean; resetKey: number;
  onSave: (kind: TicketColorKind, value: string) => Promise<void>; onChoose: (kind: TicketColorKind) => void;
}) {
  const [draft, setDraft] = useState(value), [error, setError] = useState("");
  useEffect(() => { setDraft(value); setError(""); }, [value, resetKey]);
  const preview = normalizeHexColor(draft) ?? value;
  const apply = async () => {
    const color = normalizeHexColor(draft);
    if (!color) { setError("请输入 3 或 6 位 HEX 色值，例如 #3F766E。"); return; }
    try { await onSave(kind, color); setDraft(color); setError(""); }
    catch { /* 保存失败信息由设置区统一显示，保留当前输入供重试。 */ }
  };
  return <div className="ticket-color-editor">
    <div className="ticket-color-editor-heading"><strong>{TICKET_COLOR_LABELS[kind]}</strong><span className="ticket-color-preview" style={{ backgroundColor: preview, color: ticketTextColor(preview) }} aria-label={`${TICKET_COLOR_LABELS[kind]}预览`}>01</span></div>
    <label className="ticket-color-code"><span>HEX 编码</span><div><input aria-label={`${TICKET_COLOR_LABELS[kind]} HEX 编码`} value={draft} disabled={busy} maxLength={16} spellCheck={false} aria-invalid={Boolean(error)} aria-describedby={error ? `ticket-${kind}-error` : undefined}
      onChange={event => { setDraft(event.target.value); setError(""); }} onKeyDown={event => { if (event.key === "Enter") { event.preventDefault(); void apply(); } }} />
      <button type="button" className="button secondary small" disabled={busy} onClick={() => void apply()}>应用</button></div></label>
    {error && <p className="ticket-color-error" role="alert" id={`ticket-${kind}-error`}>{error}</p>}
    <button type="button" className="button secondary small ticket-color-choose" disabled={busy} onClick={() => onChoose(kind)}><Palette size={14} />选择候选色</button>
  </div>;
}

export default function TicketColorSetting({ notify }: { notify: (text: string) => void }) {
  const { colors, unavailable, save } = useTicketColors();
  const [open, setOpen] = useState<TicketColorKind | null>(null), [busy, setBusy] = useState(false);
  const [error, setError] = useState(""), [resetKey, setResetKey] = useState(0);
  const dialog = useRef<HTMLElement>(null), trigger = useRef<HTMLElement | null>(null);
  const close = () => { setOpen(null); trigger.current?.focus(); };
  useEffect(() => {
    if (!open) return;
    const initialChoice = dialog.current?.querySelector<HTMLButtonElement>('button[aria-pressed="true"]')
      ?? dialog.current?.querySelector<HTMLButtonElement>(".ticket-color-options button");
    initialChoice?.focus();
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape" && !busy) close(); };
    window.addEventListener("keydown", escape);
    return () => window.removeEventListener("keydown", escape);
  }, [open, busy]);
  const persist = async (kind: TicketColorKind, color: string) => {
    setBusy(true); setError("");
    try { await save({ ...colors, [kind]: color }); notify(`${TICKET_COLOR_LABELS[kind]}配色已更新`); }
    catch (reason) { setError("保存失败：" + String(reason)); throw reason; }
    finally { setBusy(false); }
  };
  const reset = async () => {
    setBusy(true); setError("");
    try { await save({ ...DEFAULT_TICKET_COLORS }); setResetKey(key => key + 1); notify("已恢复默认编号配色"); }
    catch (reason) { setError("恢复默认失败：" + String(reason)); }
    finally { setBusy(false); }
  };
  const choose = async (color: string) => {
    if (!open) return;
    try { await persist(open, color); close(); } catch { /* 对话框保留，显示失败原因。 */ }
  };
  return <>
    <section className="ticket-color-setting" aria-labelledby="ticket-color-setting-title">
      <header><div><h2 id="ticket-color-setting-title">编号配色</h2><p>选择候选色或输入 HEX 编码；应用后所有窗口同步，编号字色自动适配。</p></div><button type="button" className="button secondary small" disabled={busy} onClick={() => void reset()}><RotateCcw size={15} />恢复默认</button></header>
      {unavailable && <p className="font-fallback-note">暂时无法读取配色设置，当前使用默认色。</p>}
      {error && !open && <p className="ticket-color-error" role="alert">{error}</p>}
      <div className="ticket-color-editors">{(Object.keys(DEFAULT_TICKET_COLORS) as TicketColorKind[]).map(kind => <ColorEditor key={kind} kind={kind} value={colors[kind]} busy={busy} resetKey={resetKey} onSave={persist} onChoose={value => { trigger.current = document.activeElement as HTMLElement; setError(""); setOpen(value); }} />)}</div>
    </section>
    {open && <div className="modal-layer nested-modal" onMouseDown={event => { if (event.target === event.currentTarget && !busy) close(); }}><section className="ticket-color-picker" ref={dialog} role="dialog" aria-modal="true" aria-labelledby="ticket-color-picker-title">
      <header><div><h2 id="ticket-color-picker-title">{TICKET_COLOR_LABELS[open]}配色</h2><p>16 个官方候选色 · 点击后应用</p></div><button type="button" className="icon-button" disabled={busy} aria-label="关闭配色选择" onClick={close}><X size={18} /></button></header>
      {error && <p className="ticket-color-error" role="alert">{error}</p>}
      <div className="ticket-color-options" role="group" aria-label="官方候选色">{OFFICIAL_TICKET_COLORS.map((color, index) => <button type="button" key={color.hex} disabled={busy} aria-pressed={colors[open] === color.hex} aria-label={`${color.name} ${color.hex}`} onClick={() => void choose(color.hex)}>
        <span className="ticket-color-preview" style={{ backgroundColor: color.hex, color: ticketTextColor(color.hex) }}>01</span><strong>{index + 1} · {color.name}{colors[open] === color.hex && <Check size={13} />}</strong><small>{color.hex}</small>
      </button>)}</div>
      <footer><button type="button" className="button secondary small" disabled={busy} onClick={close}>取消</button></footer>
    </section></div>}
  </>;
}
