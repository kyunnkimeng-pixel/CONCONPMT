import { describe, expect, it, vi } from "vitest";

import { approveExportEditorExit } from "@/features/export/components/ExportDialog";

describe("ExportDialog embedded editor exit approval", () => {
  it("blocks closing while the embedded editor is committing", () => {
    const confirmDiscard = vi.fn(() => true);

    expect(
      approveExportEditorExit({
        isBusy: true,
        hasUnsavedChanges: false,
        confirmDiscard,
      }),
    ).toBe(false);
    expect(confirmDiscard).not.toHaveBeenCalled();
  });

  it("closes clean state without confirmation", () => {
    const confirmDiscard = vi.fn(() => false);

    expect(
      approveExportEditorExit({
        isBusy: false,
        hasUnsavedChanges: false,
        confirmDiscard,
      }),
    ).toBe(true);
    expect(confirmDiscard).not.toHaveBeenCalled();
  });

  it("uses the user's decision before discarding embedded editor changes", () => {
    expect(
      approveExportEditorExit({
        isBusy: false,
        hasUnsavedChanges: true,
        confirmDiscard: () => false,
      }),
    ).toBe(false);
    expect(
      approveExportEditorExit({
        isBusy: false,
        hasUnsavedChanges: true,
        confirmDiscard: () => true,
      }),
    ).toBe(true);
  });
});
