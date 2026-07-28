// @vitest-environment happy-dom

import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";

import { GifAiWebExportActions } from "@/features/sheets/components/GifFrameSheetDialog";

const actEnvironment = globalThis as typeof globalThis & {
  IS_REACT_ACT_ENVIRONMENT?: boolean;
};

afterEach(() => {
  delete actEnvironment.IS_REACT_ACT_ENVIRONMENT;
});

describe("GIF manual web AI handoff", () => {
  it("copies the prompt, opens the selected official site, and continues to reimport", async () => {
    actEnvironment.IS_REACT_ACT_ENVIRONMENT = true;
    const host = document.createElement("div");
    document.body.append(host);
    const root = createRoot(host);
    const writeText = vi.fn().mockResolvedValue(undefined);
    const openAiSite = vi.fn().mockResolvedValue(undefined);
    const continueToReimport = vi.fn();
    const clipboardDescriptor = Object.getOwnPropertyDescriptor(
      navigator,
      "clipboard",
    );
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });

    try {
      await act(async () => {
        root.render(
          <GifAiWebExportActions
            prompt="PNG만 반환하세요."
            onContinueToReimport={continueToReimport}
            onOpenAiSite={openAiSite}
          />,
        );
      });

      const button = host.querySelector<HTMLButtonElement>(
        '[data-testid="gif-ai-open-gemini"]',
      );
      expect(button).not.toBeNull();
      await act(async () => {
        button?.click();
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(writeText).toHaveBeenCalledWith("PNG만 반환하세요.");
      expect(openAiSite).toHaveBeenCalledWith("gemini_ai_studio");
      expect(continueToReimport).toHaveBeenCalledTimes(1);
    } finally {
      act(() => root.unmount());
      host.remove();
      if (clipboardDescriptor) {
        Object.defineProperty(navigator, "clipboard", clipboardDescriptor);
      } else {
        Reflect.deleteProperty(navigator, "clipboard");
      }
    }
  });
});
