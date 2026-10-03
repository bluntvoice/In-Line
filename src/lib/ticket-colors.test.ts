import { describe, expect, it } from "vitest";
import { DEFAULT_TICKET_COLORS, normalizeHexColor, OFFICIAL_TICKET_COLORS, resolveTicketColors, ticketTextColor } from "./ticket-colors";

describe("ticket color configuration", () => {
  it("normalizes optional hash, case and short HEX while rejecting CSS and invalid input", () => {
    expect(normalizeHexColor(" 3f766e ")).toBe("#3F766E");
    expect(normalizeHexColor("#f3d98b")).toBe("#F3D98B");
    expect(normalizeHexColor("abc")).toBe("#AABBCC");
    expect(normalizeHexColor("#fff")).toBe("#FFFFFF");
    for (const value of ["", "#12", "#12345", "#12345678", "##fff", "#ff00gg", "red", "rgb(0,0,0)", "url(x)", "#FFéFFF"]) {
      expect(normalizeHexColor(value), value).toBeNull();
    }
  });
  it("safely defaults missing, damaged and incomplete saved preferences", () => {
    for (const value of [null, undefined, [], "broken", "null", "[]", "{}", '{"normal":"#fff"}', JSON.stringify({ ...DEFAULT_TICKET_COLORS, extra: "#fff" }), JSON.stringify({ ...DEFAULT_TICKET_COLORS, future: "red" })]) {
      expect(resolveTicketColors(value)).toEqual(DEFAULT_TICKET_COLORS);
    }
    expect(resolveTicketColors(JSON.stringify({ normal: "abc", future: "#f3d98b", urgent: "a7446a" })))
      .toEqual({ normal: "#AABBCC", future: "#F3D98B", urgent: "#A7446A" });
  });
  it("includes the approved 16 distinct colors and all three defaults", () => {
    expect(OFFICIAL_TICKET_COLORS).toHaveLength(16);
    const colors = new Set(OFFICIAL_TICKET_COLORS.map(color => color.hex));
    expect(colors.size).toBe(16);
    for (const value of Object.values(DEFAULT_TICKET_COLORS)) expect(colors.has(value as typeof OFFICIAL_TICKET_COLORS[number]["hex"])).toBe(true);
    expect(colors.has("#F3D98B")).toBe(true);
  });
  it("chooses dark text on light colors and white text on dark colors", () => {
    expect(ticketTextColor("#F3D98B")).toBe("#000000");
    expect(ticketTextColor("#CCE3DE")).toBe("#000000");
    expect(ticketTextColor("#FFFFFF")).toBe("#000000");
    expect(ticketTextColor("#000000")).toBe("#FFFFFF");
    expect(ticketTextColor("#3F766E")).toBe("#FFFFFF");
  });
  it("keeps text contrast above 4.5 across official and arbitrary custom RGB colors", () => {
    const custom = Array.from({ length: 64 }, (_, i) => "#" + ((i * 2654435761) & 0xffffff).toString(16).padStart(6, "0"));
    for (const hex of [...OFFICIAL_TICKET_COLORS.map(color => color.hex), ...custom]) {
      const rgb = hex.slice(1).match(/../g)!.map(v => parseInt(v, 16) / 255).map(v => v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
      const luminance = rgb.reduce((total, value, i) => total + value * [0.2126, 0.7152, 0.0722][i], 0);
      const contrast = ticketTextColor(hex) === "#000000" ? (luminance + 0.05) / 0.05 : 1.05 / (luminance + 0.05);
      expect(contrast, hex).toBeGreaterThanOrEqual(4.5);
    }
  });
});
