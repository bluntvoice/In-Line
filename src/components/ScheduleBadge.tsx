import type { LegalTask } from "../types";
import { scheduleLabel } from "../lib/scheduling";
export default function ScheduleBadge({task}:{task:LegalTask}){
  const label=scheduleLabel(task);
  return label?<span className="schedule-mark">{label}</span>:null;
}
