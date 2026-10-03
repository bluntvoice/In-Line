export const UI_SCALES=[100,110,120,130,140,150] as const;
export type UIScale=typeof UI_SCALES[number];
export function resolveUIScale(value:unknown):UIScale{
  return UI_SCALES.includes(Number(value) as UIScale)?Number(value) as UIScale:100;
}
