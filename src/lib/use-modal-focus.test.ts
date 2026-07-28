import { describe, expect, it } from "vitest";

import {
  isTopmostModalFocusToken,
  shouldRestoreModalFocus,
} from "@/lib/use-modal-focus";

describe("modal focus restoration policy", () => {
  it("keeps the existing trigger restoration behavior by default", () => {
    expect(shouldRestoreModalFocus(true, false)).toBe(true);
  });

  it("lets an external reveal or open handoff suppress cleanup restoration", () => {
    expect(shouldRestoreModalFocus(true, true)).toBe(false);
  });

  it("honors a modal-wide restore opt-out independently of suppression", () => {
    expect(shouldRestoreModalFocus(false, false)).toBe(false);
    expect(shouldRestoreModalFocus(false, true)).toBe(false);
  });

  it("lets only the topmost nested modal own keyboard handling", () => {
    const outer = Symbol("outer");
    const inner = Symbol("inner");

    expect(isTopmostModalFocusToken([outer, inner], outer)).toBe(false);
    expect(isTopmostModalFocusToken([outer, inner], inner)).toBe(true);
    expect(isTopmostModalFocusToken([outer], outer)).toBe(true);
  });
});