import { describe, expect, it } from "vitest";

import {
  activateIconRequestLifecycle,
  captureIconRequest,
  createIconRequestLifecycle,
  effectPreviewRequestKey,
  hasUnsavedEditorChanges,
  invalidateIconRequestLifecycle,
  isAiSourceRepairRequired,
  isEditorStateResponseCurrent,
  isIconRequestCurrent,
  isRevisionConflict,
  isStaleMeasurement,
  isTextOverlayDraftDirty,
} from "@/features/editor/editor-state-guards";
import type { TextOverlaySettings } from "@/features/editor/types";

const savedText: TextOverlaySettings = {
  enabled: true,
  text: "저장됨",
  fontPath: null,
  fontSize: 24,
  x: 0.5,
  y: 0.75,
  color: "#FFFFFFFF",
  strokeColor: "#000000FF",
  strokeWidth: 2,
};

describe("editor state guards", () => {
  it("detects text drafts without treating rounded saved positions as dirty", () => {
    const cleanDraft = {
      enabled: true,
      text: "저장됨",
      fontPath: "",
      fontSize: 24,
      xPercent: 50,
      yPercent: 75,
      color: "#FFFFFFFF",
      strokeColor: "#000000FF",
      strokeWidth: 2,
    };

    expect(isTextOverlayDraftDirty(cleanDraft, savedText)).toBe(false);
    expect(
      isTextOverlayDraftDirty(
        { ...cleanDraft, text: "저장 전 변경" },
        savedText,
      ),
    ).toBe(true);
  });

  it("guards either main or advanced unsaved state", () => {
    expect(hasUnsavedEditorChanges(false, false)).toBe(false);
    expect(hasUnsavedEditorChanges(true, false)).toBe(true);
    expect(hasUnsavedEditorChanges(false, true)).toBe(true);
  });

  it("recognizes backend revision conflicts without matching unrelated errors", () => {
    expect(isRevisionConflict({ code: "conflict" })).toBe(true);
    expect(isRevisionConflict({ code: "validation" })).toBe(false);
    expect(isRevisionConflict(new Error("conflict"))).toBe(false);
  });

  it("recognizes stale render measurements separately from revision conflicts", () => {
    expect(isStaleMeasurement({ code: "stale_measurement" })).toBe(true);
    expect(isStaleMeasurement({ code: "conflict" })).toBe(false);
    expect(isRevisionConflict({ code: "stale_measurement" })).toBe(false);
    expect(isStaleMeasurement("stale_measurement")).toBe(false);
  });

  it("invalidates preview freshness for base edits as well as recipe edits", () => {
    const base = {
      iconId: "icon_1",
      iconUpdatedAt: "2026-07-25T10:00:00Z",
      effectRevision: 2,
      draftSignature: "recipe-a",
    };

    expect(effectPreviewRequestKey(base)).not.toBe(
      effectPreviewRequestKey({
        ...base,
        iconUpdatedAt: "2026-07-25T10:00:01Z",
      }),
    );
    expect(effectPreviewRequestKey(base)).not.toBe(
      effectPreviewRequestKey({ ...base, draftSignature: "recipe-b" }),
    );
  });

  it("offers AI repair only for the exact repair-required command code", () => {
    expect(isAiSourceRepairRequired({ code: "ai_source_repair_required" })).toBe(true);
    expect(isAiSourceRepairRequired({ code: "ai_revision_conflict" })).toBe(false);
    expect(isAiSourceRepairRequired(new Error("AI source failed"))).toBe(false);
  });

  it("drops a delayed response after the editor switches icons", async () => {
    const lifecycle = createIconRequestLifecycle("icon_1");
    const token = captureIconRequest(lifecycle);
    const deferred = createDeferred<string>();
    const applied: string[] = [];
    const response = deferred.promise.then((value) => {
      if (isIconRequestCurrent(lifecycle, token)) {
        applied.push(value);
      }
    });

    activateIconRequestLifecycle(lifecycle, "icon_2");
    deferred.resolve("old icon response");
    await response;

    expect(applied).toEqual([]);
    expect(isEditorStateResponseCurrent("icon_2", "icon_1")).toBe(false);
    expect(isEditorStateResponseCurrent("icon_2", "icon_2")).toBe(true);
  });

  it("drops a delayed response after the editor unmounts", async () => {
    const lifecycle = createIconRequestLifecycle("icon_1");
    const token = captureIconRequest(lifecycle);
    const deferred = createDeferred<string>();
    const applied: string[] = [];
    const response = deferred.promise.then((value) => {
      if (isIconRequestCurrent(lifecycle, token)) {
        applied.push(value);
      }
    });

    invalidateIconRequestLifecycle(lifecycle);
    deferred.resolve("unmounted response");
    await response;

    expect(applied).toEqual([]);
  });

  it("keeps a delayed AI mutation failure from clearing the next icon state", async () => {
    const lifecycle = createIconRequestLifecycle("icon_1");
    const token = captureIconRequest(lifecycle);
    const deferred = createDeferred<string>();
    let busy = true;
    let errorMessage: string | null = null;
    const response = deferred.promise
      .catch((error: unknown) => {
        if (isIconRequestCurrent(lifecycle, token)) {
          errorMessage = error instanceof Error ? error.message : "unknown";
        }
      })
      .finally(() => {
        if (isIconRequestCurrent(lifecycle, token)) {
          busy = false;
        }
      });

    activateIconRequestLifecycle(lifecycle, "icon_2");
    busy = true;
    deferred.reject(new Error("old icon failure"));
    await response;

    expect(errorMessage).toBeNull();
    expect(busy).toBe(true);
  });
});

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });
  return { promise, reject, resolve };
}
