import { Check, ChevronDown, X } from "lucide-react";
import { useEffect, useId, useMemo, useRef, useState } from "react";

interface Props {
  values: string[];
  options: string[];
  commonContacts: string[];
  onChange: (values: string[]) => void;
  onDelete?: (value: string) => Promise<void> | void;
}

const cleanValues = (values: string[]) => [...new Set(values.map(value => value.trim()).filter(Boolean))];

export default function MultiContactInput({ values, options, commonContacts, onChange, onDelete }: Props) {
  const [draft, setDraft] = useState("");
  const [open, setOpen] = useState(false);
  const [deleting, setDeleting] = useState("");
  const root = useRef<HTMLDivElement>(null);
  const listId = useId();
  const selected = useMemo(() => cleanValues(values), [values]);
  const visible = useMemo(() => {
    const unique = cleanValues(options);
    const keyword = draft.trim().toLocaleLowerCase("zh-CN");
    return keyword ? unique.filter(item => item.toLocaleLowerCase("zh-CN").includes(keyword)) : unique;
  }, [draft, options]);
  const isNewValue = Boolean(draft.trim()) && !options.some(item => item === draft.trim());

  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, [open]);

  const add = (value: string) => {
    const name = value.trim();
    if (!name || selected.includes(name) || selected.length >= 10) {
      setDraft("");
      return;
    }
    onChange([...selected, name]);
    setDraft("");
  };
  const remove = (value: string) => onChange(selected.filter(item => item !== value));
  const deleteOption = async (value: string) => {
    if (!onDelete || deleting) return;
    setDeleting(value);
    try {
      await onDelete(value);
    } finally {
      setDeleting("");
    }
  };

  return <div className="multi-contact" ref={root}>
    <div className="multi-contact-control" onClick={() => root.current?.querySelector("input")?.focus()}>
      {selected.map(name => <span className="contact-chip" key={name}>{name}<button type="button" aria-label={`移除对接人“${name}”`} onClick={event => { event.stopPropagation(); remove(name); }}><X size={13} /></button></span>)}
      <input value={draft} placeholder={selected.length ? "继续添加" : "输入或选择对接人"} role="combobox" aria-autocomplete="list" aria-expanded={open} aria-controls={listId}
        onFocus={() => setOpen(true)} onChange={event => { setDraft(event.target.value); setOpen(true); }} onBlur={() => add(draft)}
        onKeyDown={event => {
          if (event.key === "Enter" || event.key === "," || event.key === "，") { event.preventDefault(); event.stopPropagation(); add(draft); setOpen(true); }
          if (event.key === "Backspace" && !draft && selected.length) remove(selected[selected.length - 1]);
          if (event.key === "Escape") setOpen(false);
          if (event.key === "ArrowDown") { event.preventDefault(); setOpen(true); }
        }} />
      <button type="button" className="multi-contact-toggle" aria-label="展开对接人选项" onPointerDown={event => event.preventDefault()} onClick={event => { event.stopPropagation(); setOpen(current => !current); }}><ChevronDown size={16} /></button>
    </div>
    {open && <div className="combo-options multi-contact-options" id={listId} role="listbox">
      {visible.map(item => <div className="combo-option" role="option" aria-selected={selected.includes(item)} key={item}>
        <button type="button" className="combo-select" onPointerDown={event => event.preventDefault()} onClick={() => { add(item); setOpen(true); }}>
          <span>{item}</span>{selected.includes(item) && <Check size={15} />}
        </button>
        {onDelete && <button type="button" className="combo-delete" disabled={deleting === item} title={`删除“${item}”`} aria-label={`删除“${item}”`}
          onPointerDown={event => event.preventDefault()} onClick={event => { event.stopPropagation(); void deleteOption(item); }}><X size={14} /></button>}
      </div>)}
      {isNewValue && <button type="button" className="multi-contact-add" onPointerDown={event => event.preventDefault()} onClick={() => add(draft)}>添加“{draft.trim()}”</button>}
      {!visible.length && !isNewValue && <div className="combo-empty">暂无可选项，可直接输入新联系人</div>}
    </div>}
    {commonContacts.length > 0 && <span className="recent-contacts">{commonContacts.map(name => <button type="button" disabled={selected.includes(name)} key={name} onClick={() => add(name)}>{name}</button>)}</span>}
  </div>;
}
