import type { LegalTask } from "../types";
import { dayDifference, displayTicket, isOverdue } from "../lib/task-utils";

export default function TicketNumber({ task, showPermanent = false }: { task: LegalTask; showPermanent?: boolean }) {
  const age = dayDifference(task.ticketDate);
  const alert = isOverdue(task) || task.priority === "critical";
  const ticket = displayTicket(task);
  const match = ticket.match(/^([A-Z]*)(\d+)$/);
  return (
    <span className="ticket-identity">
      <span
        className={`ticket-number ${age > 0 ? "ticket-aged" : ""} ${alert ? "ticket-alert" : ""}`}
        title={task.hasActiveQueue ? (age > 0 ? `该事项已在队列中等候 ${age} 天，请关注时效风险。` : "今日取号事项") : "最近一次排队序号"}
      >
        {alert && <span className="ticket-triangle" aria-label="时效警示">▲</span>}
        <span className="ticket-prefix">{match?.[1]}</span>
        <strong>{match?.[2] ?? ticket}</strong>
      </span>
      {showPermanent && <span className="permanent-number">事项编号：{task.permanentNumber}</span>}
    </span>
  );
}
