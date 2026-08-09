type ShortcutKeyboardInput=Pick<KeyboardEvent,"code"|"ctrlKey"|"altKey"|"shiftKey"|"metaKey">;

const NAMED_KEYS:Record<string,string>={
  Space:"Space",Enter:"Enter",Tab:"Tab",ArrowUp:"ArrowUp",ArrowDown:"ArrowDown",ArrowLeft:"ArrowLeft",ArrowRight:"ArrowRight",
  Backquote:"`",Minus:"-",Equal:"=",BracketLeft:"[",BracketRight:"]",Backslash:"\\",Semicolon:";",Quote:"'",Comma:",",Period:".",Slash:"/"
};

export interface ShortcutCaptureResult {shortcut?:string;error?:string}

function mainKey(code:string){
  if(/^Key[A-Z]$/.test(code))return code.slice(3);
  if(/^Digit[0-9]$/.test(code))return code.slice(5);
  if(/^F([1-9]|1[0-2])$/.test(code))return code;
  return NAMED_KEYS[code];
}

export function shortcutFromKeyboardEvent(event:ShortcutKeyboardInput):ShortcutCaptureResult{
  if(event.metaKey)return{error:"Windows 键组合由系统保留，请改用 Ctrl 或 Alt"};
  const key=mainKey(event.code);
  if(!key)return{};
  if(!event.ctrlKey&&!event.altKey)return{error:"请至少同时按住 Ctrl 或 Alt"};
  const parts:string[]=[];
  if(event.ctrlKey)parts.push("Ctrl");
  if(event.altKey)parts.push("Alt");
  if(event.shiftKey)parts.push("Shift");
  parts.push(key);
  const shortcut=parts.join("+");
  if(shortcut==="Alt+F4"||shortcut==="Alt+Shift+F4")return{error:"Alt+F4 是 Windows 关闭窗口快捷键，不能使用"};
  return{shortcut};
}

export function shortcutUsageHint(shortcut:string){
  if(/^Ctrl\+[A-Z0-9]$/.test(shortcut))return "已通过系统全局注册检查；该组合也可能是其他软件的常用快捷键，使用时会优先打开新增取号。";
  return "已通过系统全局注册检查，快捷键可以使用。";
}
