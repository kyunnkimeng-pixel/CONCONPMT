import { describe, expect, it } from "vitest";

import {
  ADVANCED_EDITOR_MODES,
  nextAdvancedEditorMode,
} from "@/features/editor/advanced-editor-tabs";

describe("advanced editor tabs", () => {
  it("keeps the fixed static, motion and text-capacity order", () => {
    expect(ADVANCED_EDITOR_MODES).toEqual(["effects", "motion", "tools"]);
  });

  it("moves with horizontal tab keys and wraps at both ends", () => {
    expect(nextAdvancedEditorMode("effects", "ArrowRight")).toBe("motion");
    expect(nextAdvancedEditorMode("motion", "ArrowRight")).toBe("tools");
    expect(nextAdvancedEditorMode("tools", "ArrowRight")).toBe("effects");
    expect(nextAdvancedEditorMode("effects", "ArrowLeft")).toBe("tools");
    expect(nextAdvancedEditorMode("tools", "ArrowLeft")).toBe("motion");
  });

  it("supports Home and End without intercepting unrelated keys", () => {
    expect(nextAdvancedEditorMode("motion", "Home")).toBe("effects");
    expect(nextAdvancedEditorMode("motion", "End")).toBe("tools");
    expect(nextAdvancedEditorMode("motion", "Enter")).toBeNull();
    expect(nextAdvancedEditorMode("motion", "Tab")).toBeNull();
  });
});
