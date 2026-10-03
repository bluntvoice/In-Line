export type TicketColorKind = "normal" | "future" | "urgent";
export type TicketColors = Record<TicketColorKind, string>;
export const DEFAULT_TICKET_COLORS: TicketColors = {
  normal: "#0B3A82", future: "#3F766E", urgent: "#C43D4B"
};
export const TICKET_COLOR_LABELS: Record<TicketColorKind, string> = {
  normal: "普通编号", future: "未来编号", urgent: "加急编号"
};
export const OFFICIAL_TICKET_COLORS = [
  { name: "深蓝", hex: "#0B3A82" }, { name: "灰绿", hex: "#3F766E" },
  { name: "警示红", hex: "#C43D4B" }, { name: "雾蓝", hex: "#536C8F" },
  { name: "灰靛", hex: "#666B91" }, { name: "石板灰", hex: "#68717D" },
  { name: "灰紫", hex: "#7A6687" }, { name: "枣红", hex: "#9B4055" },
  { name: "赭橙", hex: "#A35B2E" }, { name: "莓红", hex: "#A7446A" },
  { name: "湖蓝", hex: "#3E788A" }, { name: "松绿", hex: "#456F53" },
  { name: "咖啡", hex: "#80664E" }, { name: "豆沙", hex: "#945F64" },
  { name: "奶油黄", hex: "#F3D98B" }, { name: "浅青", hex: "#CCE3DE" }
] as const;

export function normalizeHexColor(input: string): string | null {
  const hex = input.trim().replace(/^#/, "");
  if (!/^(?:[0-9a-f]{3}|[0-9a-f]{6})$/i.test(hex)) return null;
  return "#" + (hex.length === 3 ? [...hex].map(char => char + char).join("") : hex).toUpperCase();
}

export function resolveTicketColors(stored: unknown): TicketColors {
  try {
    if (typeof stored !== "string") return { ...DEFAULT_TICKET_COLORS };
    const parsed: unknown = JSON.parse(stored);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return { ...DEFAULT_TICKET_COLORS };
    const colors = parsed as Record<string, unknown>;
    const entries = Object.keys(DEFAULT_TICKET_COLORS).map(kind => {
      const value = colors[kind];
      return [kind, typeof value === "string" ? normalizeHexColor(value) : null] as const;
    });
    if (entries.some(([, color]) => !color) || Object.keys(colors).length !== 3) return { ...DEFAULT_TICKET_COLORS };
    return Object.fromEntries(entries) as TicketColors;
  } catch {
    return { ...DEFAULT_TICKET_COLORS };
  }
}

export function ticketTextColor(hex: string): "#000000" | "#FFFFFF" {
  const color = normalizeHexColor(hex) ?? DEFAULT_TICKET_COLORS.normal;
  const rgb = [1, 3, 5].map(offset => parseInt(color.slice(offset, offset + 2), 16) / 255)
    .map(value => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4);
  const luminance = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
  return 1.05 / (luminance + 0.05) >= (luminance + 0.05) / 0.05 ? "#FFFFFF" : "#000000";
}

export function applyTicketColors(colors: TicketColors) {
  for (const kind of Object.keys(DEFAULT_TICKET_COLORS) as TicketColorKind[]) {
    document.documentElement.style.setProperty(`--ticket-${kind}-background`, colors[kind]);
    document.documentElement.style.setProperty(`--ticket-${kind}-text`, ticketTextColor(colors[kind]));
  }
}
