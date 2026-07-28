import { describe, expect, it, vi } from "vitest";

import {
  focusRevealedEditorPanel,
  shouldHandleIconRevealRequest,
  type IconRevealRequest,
} from "@/features/icons/icon-reveal";

describe("icon reveal request", () => {
  const request: IconRevealRequest = {
    iconId: "icon_2",
    action: "focus_tile",
    requestId: 1,
  };

  it("waits until the requested icon exists in the ordered grid", () => {
    expect(shouldHandleIconRevealRequest(request, null, ["icon_1"])).toBe(false);
    expect(
      shouldHandleIconRevealRequest(request, null, ["icon_1", "icon_2"]),
    ).toBe(true);
  });

  it("consumes one request id once but accepts a newer request for the same icon", () => {
    expect(
      shouldHandleIconRevealRequest(request, request.requestId, ["icon_2"]),
    ).toBe(false);
    expect(
      shouldHandleIconRevealRequest(
        {
          ...request,
          action: "open_editor",
          requestId: request.requestId + 1,
        },
        request.requestId,
        ["icon_2"],
      ),
    ).toBe(true);
  });

  it("ignores an empty host request", () => {
    expect(shouldHandleIconRevealRequest(null, null, ["icon_2"])).toBe(false);
  });
});

describe("revealed editor focus", () => {
  it("focuses the editor close button only when the new panel exists", () => {
    const focus = vi.fn();
    const rootWithPanel = {
      querySelector: () => ({ focus }),
    } as unknown as ParentNode;
    const rootWithoutPanel = {
      querySelector: () => null,
    } as unknown as ParentNode;

    expect(focusRevealedEditorPanel(rootWithPanel)).toBe(true);
    expect(focus).toHaveBeenCalledOnce();
    expect(focusRevealedEditorPanel(rootWithoutPanel)).toBe(false);
  });
});