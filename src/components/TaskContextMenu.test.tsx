import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { LegalTask } from "../types";
import TaskContextMenu from "./TaskContextMenu";

const task = {
  id: 2,
  title: "未入队的子任务",
  parentTaskId: 1,
  status: "pending",
  hasActiveQueue: false,
  archivedAt: null,
  isUrgent: false
} as LegalTask;

function menuFor(overrides: Partial<LegalTask> = {}) {
  vi.stubGlobal("window", { innerWidth: 1200, innerHeight: 800 });
  return renderToStaticMarkup(
    <TaskContextMenu
      task={{ ...task, ...overrides }}
      view="queue"
      x={100}
      y={100}
      onAction={() => undefined}
      onClose={() => undefined}
    />
  );
}

describe("task context menu queue action", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("offers enqueue for a pending subtask created without a queue entry", () => {
    expect(menuFor()).toContain("加入今日队列");
  });

  it("does not offer another queue entry to a task already in the queue", () => {
    expect(menuFor({ hasActiveQueue: true })).not.toContain("加入今日队列");
  });
});
