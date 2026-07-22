import type { LegalTask } from "../types";
import { displayTicket, formatDeadline, queueAheadMessage, STATUS_LABELS } from "./task-utils";

const WIDTH = 800;
const HEIGHT = 980;
const PIXEL_RATIO = 2;
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
export function fitTextLines(text: string, maxWidth: number, maxLines: number, measure: (value: string) => number) {
  const content = text.trim();
  const lines: string[] = [];
  let cursor = 0;
  for (let lineIndex = 0; lineIndex < maxLines && cursor < content.length; lineIndex += 1) {
    let line = "";
    while (cursor < content.length) {
      const next = line + content[cursor];
      if (line && measure(next) > maxWidth) break;
      line = next;
      cursor += 1;
      if (measure(line) > maxWidth) break;
    }
    if (lineIndex === maxLines - 1 && cursor < content.length) {
      while (line && measure(`${line}…`) > maxWidth) line = line.slice(0, -1);
      line = `${line}…`;
    }
    lines.push(line);
  }
  return { lines, truncated: cursor < content.length };
}
function fitLines(context: CanvasRenderingContext2D, text: string, maxWidth: number, maxLines = 2) {
  return fitTextLines(text, maxWidth, maxLines, value => context.measureText(value).width);
}
function titleLayout(context: CanvasRenderingContext2D, title: string) {
  for (let size = 40; size >= 28; size -= 2) {
    context.font = `700 ${size}px ${UI_FONT}`;
    const layout = fitLines(context, title, 668, 2);
    if (!layout.truncated || size === 28) return { ...layout, size, lineHeight: Math.round(size * 1.3) };
  }
  return { lines: [title], truncated: false, size: 28, lineHeight: 36 };
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

interface TicketRenderer { canvas: HTMLCanvasElement; context: CanvasRenderingContext2D }

const PNG_CACHE_LIMIT = 8;
const pngCache = new Map<string, Promise<Uint8Array>>();
let renderer: TicketRenderer | undefined;
let fontsReady: Promise<unknown> | undefined;
let renderQueue: Promise<void> = Promise.resolve();

function getRenderer(): TicketRenderer {
  if (renderer) return renderer;
  const canvas = document.createElement("canvas");
  canvas.width = WIDTH * PIXEL_RATIO;
  canvas.height = HEIGHT * PIXEL_RATIO;
  const context = canvas.getContext("2d", { alpha: false, willReadFrequently: true });
  if (!context) throw new Error("当前设备无法生成取号图片");
  renderer = { canvas, context };
  return renderer;
}

function waitForFonts() {
  fontsReady ??= document.fonts.ready;
  return fontsReady;
}

function enqueueRender<T>(job: () => Promise<T> | T): Promise<T> {
  const result = renderQueue.then(job, job);
  renderQueue = result.then(() => undefined, () => undefined);
  return result;
}

export function ticketRenderKey(task: LegalTask, queueAhead = 0) {
  return JSON.stringify([
    displayTicket(task), task.title, task.status, task.department, task.contact,
    task.taskType, task.requestedDeadline, task.requestedDeadlineLabel,
    task.permanentNumber, task.isUrgent, task.priority, queueAhead
  ]);
}

function drawTicket(task: LegalTask, queueAhead: number): TicketRenderer {
  const { canvas, context } = getRenderer();
  context.setTransform(PIXEL_RATIO, 0, 0, PIXEL_RATIO, 0, 0);
  context.imageSmoothingEnabled = true;
  context.imageSmoothingQuality = "high";
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
  const title = titleLayout(context, task.title);
  context.font = `700 ${title.size}px ${UI_FONT}`; context.fillStyle = COLORS.ink;
  title.lines.forEach((line, index) => context.fillText(line, 66, 516 + index * title.lineHeight));

  context.fillStyle = COLORS.line; context.fillRect(66, 620, 666, 1);
  const fields = [["部门 / 团队", task.department], ["对接人", task.contact], ["事项类型", task.taskType], ["要求完成", formatDeadline(task.requestedDeadline, task.requestedDeadlineLabel)]];
  fields.forEach(([label, value], index) => {
    const x = 66 + (index % 2) * 350;
    const y = 674 + Math.floor(index / 2) * 116;
    context.font = `500 17px ${UI_FONT}`; context.fillStyle = COLORS.muted; context.fillText(label, x, y);
    context.font = `600 24px ${UI_FONT}`; context.fillStyle = COLORS.ink;
    fitLines(context, value, 294, 2).lines.forEach((line, lineIndex) => context.fillText(line, x, y + 38 + lineIndex * 31));
  });

  context.font = "500 15px Consolas,monospace"; context.fillStyle = "#7B8797"; context.fillText(task.permanentNumber, 66, 920);
  context.font = `500 16px ${UI_FONT}`; context.textAlign = "right";
  context.fillText(queueAheadMessage(queueAhead), 732, 920);
  return { canvas, context };
}

function canvasToPng(canvas: HTMLCanvasElement) {
  return new Promise<Uint8Array>((resolve, reject) => {
    canvas.toBlob(async blob => {
      if (!blob) {
        reject(new Error("当前设备无法压缩取号图片"));
        return;
      }
      resolve(new Uint8Array(await blob.arrayBuffer()));
    }, "image/png");
  });
}

export async function warmTicketRenderer() {
  getRenderer();
  await waitForFonts();
}

export function renderTicketPng(task: LegalTask, queueAhead = 0): Promise<Uint8Array> {
  const key = ticketRenderKey(task, queueAhead);
  const cached = pngCache.get(key);
  if (cached) {
    pngCache.delete(key);
    pngCache.set(key, cached);
    return cached;
  }

  const rendered = enqueueRender(async () => {
    await waitForFonts();
    const { canvas } = drawTicket(task, queueAhead);
    return canvasToPng(canvas);
  });
  pngCache.set(key, rendered);
  if (pngCache.size > PNG_CACHE_LIMIT) {
    const oldestKey = pngCache.keys().next().value as string | undefined;
    if (oldestKey) pngCache.delete(oldestKey);
  }
  void rendered.catch(() => pngCache.delete(key));
  return rendered;
}

export async function renderTicketRgba(task: LegalTask, queueAhead = 0): Promise<TicketRgbaImage> {
  return enqueueRender(async () => {
    await waitForFonts();
    const { canvas, context } = drawTicket(task, queueAhead);
  const rgba = context.getImageData(0, 0, canvas.width, canvas.height).data;
    return {
      rgba: new Uint8Array(rgba.buffer, rgba.byteOffset, rgba.byteLength),
      width: canvas.width,
      height: canvas.height
    };
  });
}
