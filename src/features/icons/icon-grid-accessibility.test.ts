import { describe, expect, it } from "vitest";

import { resolveDndAccessibilityContainer } from "@/features/icons/icon-grid-accessibility";

describe("icon grid drag accessibility container", () => {
  it("keeps dnd-kit announcements in the document during normal grid use", () => {
    const detachedContainer = {} as Element;

    expect(
      resolveDndAccessibilityContainer(false, detachedContainer),
    ).toBeUndefined();
  });

  it("moves dnd-kit announcements to a detached container behind an AI modal", () => {
    const detachedContainer = {} as Element;

    expect(
      resolveDndAccessibilityContainer(true, detachedContainer),
    ).toBe(detachedContainer);
  });

  it("falls back safely before a browser document exists", () => {
    expect(resolveDndAccessibilityContainer(true, null)).toBeUndefined();
  });
});
