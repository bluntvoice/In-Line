import { describe, expect, it } from "vitest";
import { createSubtaskDraft, moveById, reorderById } from "./subtask-ui";

const items = [{ id: 1 }, { id: 2 }, { id: 3 }];

describe("subtask ordering", () => {
  it("drops a dragged subtask at the target position", () => {
    expect(reorderById(items, 3, 1).map(item => item.id)).toEqual([3, 1, 2]);
    expect(reorderById(items, 1, 3).map(item => item.id)).toEqual([2, 3, 1]);
  });

  it("ignores invalid drag targets", () => {
    expect(reorderById(items, 1, 99)).toBe(items);
    expect(reorderById(items, 2, 2)).toBe(items);
  });

  it("supports keyboard-friendly single-step moves", () => {
    expect(moveById(items, 2, "up").map(item => item.id)).toEqual([2, 1, 3]);
    expect(moveById(items, 2, "down").map(item => item.id)).toEqual([1, 3, 2]);
    expect(moveById(items, 1, "up")).toBe(items);
  });
});

describe("subtask creation defaults", () => {
  it("copies people and classification while keeping deadline and urgency independent", () => {
    const draft = createSubtaskDraft({
      id: 7,
      taskType: "合同审查",
      departments: ["法务部", "业务部"],
      department: "法务部、业务部",
      contacts: ["张三", "李四"],
      contact: "张三、李四"
    });
    expect(draft).toMatchObject({
      parentTaskId: 7,
      taskType: "合同审查",
      departments: ["法务部", "业务部"],
      contacts: ["张三", "李四"],
      requestedDeadline: null,
      isUrgent: false,
      enqueueToday: true
    });
  });
});
