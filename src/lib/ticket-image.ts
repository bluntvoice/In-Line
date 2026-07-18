import type { LegalTask } from "../types";
import { displayTicket, formatDeadline, queueAheadMessage, STATUS_LABELS } from "./task-utils";

const WIDTH = 800;
const HEIGHT = 980;
const COLORS = { blue: "#0B3A82", ink: "#102A56", amber: "#FFB000", paper: "#F4F7FA", muted: "#526173", red: "#C43D4B", line: "#DFE6EF" };
const UI_FONT = "'Microsoft YaHei UI','Microsoft YaHei','Segoe UI',sans-serif";

function rounded(context: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number, fill: string) {
  context.beginPath(); context.roundRect(x, y, w, h, r); context.fillStyle = fill; context.fill();
}
function pill(context: CanvasRenderingContext2D, text: string, right: number, y: number, fill: string, color = "#FFF") {
  context.font = `700 18px ${UI_FONT}`;
  const width = Math.max(108, Math.ceil(context.measureText(text).width) + 38);
  const x = right - width;
  rounded(context, x, y, width, 46, 23, fill);
  context.fillStyle = color;
  context.textAlign = "center";
  context.fillText(text, x + width / 2, y + 23);
  context.textAlign = "left";
}
function fitLines(context: CanvasRenderingContext2D, text: string, maxWidth: number, maxLines = 2) {
  const lines: string[] = []; let line = "";
  for (const char of text) {
    const next = line + char;
    if (line && context.measureText(next).width > maxWidth) { lines.push(line); line = char; if (lines.length === maxLines - 1) break; } else line = next;
  }
  if (line && lines.length < maxLines) lines.push(line);
  if (lines.join("").length < text.length) {
    let last = lines.at(-1) ?? "";
    while (last && context.measureText(last + "…").width > maxWidth) last = last.slice(0, -1);
    lines[lines.length - 1] = last + "…";
  }
  return lines;
}
function ticketFontSize(context: CanvasRenderingContext2D, ticket: string) {
  let size = 196;
  while (size > 132) {
    context.font = `800 ${size}px Consolas,'Cascadia Mono',monospace`;
    if (context.measureText(ticket).width <= 620) return size;
    size -= 4;
  }
  return size;
}
export interface TicketRgbaImage { rgba: Uint8Array; width: number; height: number }

export async function renderTicketRgba(task: LegalTask, queueAhead = 0): Promise<TicketRgbaImage> {
  await document.fonts.ready;
  const canvas = document.createElement("canvas"); canvas.width = WIDTH; canvas.height = HEIGHT;
  const context = canvas.getContext("2d"); if (!context) throw new Error("当前设备无法生成取号图片");
  context.textBaseline = "middle";
  context.fillStyle = COLORS.paper; context.fillRect(0, 0, WIDTH, HEIGHT);
  rounded(context, 24, 24, 752, 932, 32, "#FFFFFF");
  context.fillStyle = COLORS.blue; context.fillRect(24, 24, 10, 932);

  context.font = `700 27px ${UI_FONT}`; context.fillStyle = COLORS.ink; context.fillText("IN LINE", 66, 72);
  context.font = `500 18px ${UI_FONT}`; context.fillStyle = COLORS.muted; context.fillText("事项已登记", 66, 110);
  const alert = task.isUrgent || task.priority === "critical";
  pill(context, alert ? `加急 · ${STATUS_LABELS[task.status]}` : STATUS_LABELS[task.status], 732, 52, alert ? COLORS.red : COLORS.blue);

  const ticket = displayTicket(task);
  const numberSize = ticketFontSize(context, ticket);
  context.font = `800 ${numberSize}px Consolas,'Cascadia Mono',monospace`; context.fillStyle = COLORS.blue; context.textAlign = "center";
  context.fillText(ticket, 400, 258);
  context.fillStyle = COLORS.amber; context.fillRect(268, 360, 264, 10);

  const aheadText = queueAhead > 0 ? `前方还有 ${queueAhead} 项` : "当前排在队首";
  context.font = `600 22px ${UI_FONT}`;
  const aheadWidth = Math.max(190, Math.ceil(context.measureText(aheadText).width) + 48);
  rounded(context, (WIDTH - aheadWidth) / 2, 396, aheadWidth, 52, 26, "#E4ECF5");
  context.fillStyle = COLORS.blue; context.fillText(aheadText, 400, 422);

  context.textAlign = "left";
  context.font = `700 40px ${UI_FONT}`; context.fillStyle = COLORS.ink;
  fitLines(context, task.title, 668, 2).forEach((line, index) => context.fillText(line, 66, 516 + index * 52));

  context.fillStyle = COLORS.line; context.fillRect(66, 620, 666, 1);
  const fields = [["部门 / 团队", task.department], ["对接人", task.contact], ["事项类型", task.taskType], ["要求完成", formatDeadline(task.requestedDeadline, task.requestedDeadlineLabel)]];
  fields.forEach(([label, value], index) => {
    const x = 66 + (index % 2) * 350;
    const y = 674 + Math.floor(index / 2) * 116;
    context.font = `500 17px ${UI_FONT}`; context.fillStyle = COLORS.muted; context.fillText(label, x, y);
    context.font = `600 24px ${UI_FONT}`; context.fillStyle = COLORS.ink;
    fitLines(context, value, 294, 2).forEach((line, lineIndex) => context.fillText(line, x, y + 38 + lineIndex * 31));
  });

  context.font = "500 15px Consolas,monospace"; context.fillStyle = "#7B8797"; context.fillText(task.permanentNumber, 66, 920);
  context.font = `500 16px ${UI_FONT}`; context.textAlign = "right";
  context.fillText(queueAheadMessage(queueAhead), 732, 920);
  const rgba = context.getImageData(0, 0, WIDTH, HEIGHT).data;
  return { rgba: new Uint8Array(rgba), width: WIDTH, height: HEIGHT };
}
