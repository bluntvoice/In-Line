import type { TaskStatus } from "../types";
import { STATUS_LABELS } from "../lib/task-utils";
import { ClockAlert } from "lucide-react";

export default function StatusBadge({ status, overdue = false }: { status: TaskStatus; overdue?: boolean }) {
  const visualStatus = status === "waiting_counterparty_confirmation" ? "waiting_confirmation" : status;
  return (
    <span className="status-stack">
      <span className={`status-badge status-${visualStatus}`}>
        <span className="status-dot" aria-hidden="true" />
        {STATUS_LABELS[status]}
      </span>
      {overdue&&<span className="overdue-indicator" title="该事项已超过要求完成时间"><ClockAlert size={13} aria-hidden="true"/>已逾期</span>}
    </span>
  );
}
