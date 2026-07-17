import type { TaskStatus } from "../types";
import { STATUS_LABELS } from "../lib/task-utils";

export default function StatusBadge({ status, overdue = false }: { status: TaskStatus; overdue?: boolean }) {
  const className = overdue ? "status-badge status-overdue" : `status-badge status-${status}`;
  return (
    <span className={className}>
      <span className="status-dot" aria-hidden="true" />
      {overdue ? "已逾期" : STATUS_LABELS[status]}
    </span>
  );
}
