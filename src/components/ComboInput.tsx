import { Check, ChevronDown, ChevronUp, X } from "lucide-react";
import { useEffect, useId, useMemo, useRef, useState, type Ref } from "react";

interface Props {
  value: string;
  options: string[];
  placeholder?: string;
  inputRef?: Ref<HTMLInputElement>;
  onChange: (value: string) => void;
  onDelete?: (value: string) => Promise<void> | void;
  onMove?: (value: string, direction: "up" | "down") => Promise<void> | void;
}

export default function ComboInput({ value, options, placeholder, inputRef, onChange, onDelete, onMove }: Props) {
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState<string | null>(null);
  const [deleting, setDeleting] = useState("");
  const [moving, setMoving] = useState("");
  const root = useRef<HTMLDivElement>(null);
  const listId = useId();

  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, [open]);

  const visible = useMemo(() => {
    const unique = [...new Set(options.map((item) => item.trim()).filter(Boolean))];
    if (filter === null || !filter.trim()) return unique;
    const keyword = filter.trim().toLocaleLowerCase("zh-CN");
    return unique.filter((item) => item.toLocaleLowerCase("zh-CN").includes(keyword));
  }, [filter, options]);
  const isNewValue = Boolean(value.trim()) && !options.some((item) => item === value.trim());

  const remove = async (item: string) => {
    if (!onDelete || deleting) return;
    setDeleting(item);
    try {
      await onDelete(item);
    } finally {
      setDeleting("");
    }
  };
  const move = async (item: string, direction: "up" | "down") => {
    if (!onMove || moving) return;
    setMoving(item);
    try {
      await onMove(item, direction);
    } finally {
      setMoving("");
    }
  };

  return <div className="combo-input" ref={root}>
    <div className="combo-control">
      <input ref={inputRef} value={value} placeholder={placeholder} role="combobox" aria-autocomplete="list" aria-expanded={open} aria-controls={listId}
        onFocus={() => { setFilter(null); setOpen(true); }}
        onChange={(event) => { onChange(event.target.value); setFilter(event.target.value); setOpen(true); }}
        onKeyDown={(event) => {
          if (event.key === "Escape") setOpen(false);
          if (event.key === "ArrowDown") { event.preventDefault(); setFilter(null); setOpen(true); }
        }} />
      <button type="button" aria-label="展开选项" aria-expanded={open} onClick={() => { setFilter(null); setOpen((current) => !current); }}><ChevronDown size={16} /></button>
    </div>
    {open && <div className="combo-options" id={listId} role="listbox">
      {visible.map((item) => <div className="combo-option" role="option" aria-selected={item === value} key={item}>
        <button type="button" className="combo-select" onPointerDown={(event) => event.preventDefault()} onClick={() => { onChange(item); setFilter(null); setOpen(false); }}>
          <span>{item}</span>{item === value && <Check size={15} />}
        </button>
        {onMove && <span className="combo-order-actions">
          <button type="button" disabled={moving === item || options.indexOf(item) <= 0} title={`上移“${item}”`} aria-label={`上移“${item}”`}
            onPointerDown={(event) => event.preventDefault()} onClick={(event) => { event.stopPropagation(); void move(item, "up"); }}><ChevronUp size={14} /></button>
          <button type="button" disabled={moving === item || options.indexOf(item) === options.length - 1} title={`下移“${item}”`} aria-label={`下移“${item}”`}
            onPointerDown={(event) => event.preventDefault()} onClick={(event) => { event.stopPropagation(); void move(item, "down"); }}><ChevronDown size={14} /></button>
        </span>}
        {onDelete && <button type="button" className="combo-delete" disabled={deleting === item} title={`删除“${item}”`} aria-label={`删除“${item}”`}
          onPointerDown={(event) => event.preventDefault()} onClick={(event) => { event.stopPropagation(); void remove(item); }}><X size={14} /></button>}
      </div>)}
      {isNewValue && <div className="combo-new">保存后记住“{value.trim()}”</div>}
      {!visible.length && !isNewValue && <div className="combo-empty">暂无可选项，可直接输入新内容</div>}
    </div>}
  </div>;
}
