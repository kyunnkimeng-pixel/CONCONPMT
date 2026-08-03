// @vitest-environment happy-dom

import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";

import { GifAiWebExportActions } from "@/features/sheets/components/GifFrameSheetDialog";

const actEnvironment = globalThis as typeof globalThis & {
  IS_REACT_ACT_ENVIRONMENT?: boolean;
};

function deferredVoid() {
  let resolve!: () => void;
  const promise = new Promise<void>((complete) => {
    resolve = () => complete();
  });
  return { promise, resolve };
}

function setControlValue(
  control: HTMLSelectElement | HTMLTextAreaElement,
  value: string,
) {
  const descriptor = Object.getOwnPropertyDescriptor(
    Object.getPrototypeOf(control),
    "value",
  );
  descriptor?.set?.call(control, value);
  control.dispatchEvent(
    new Event(control instanceof HTMLSelectElement ? "change" : "input", {
      bubbles: true,
    }),
  );
}

afterEach(() => {
  delete actEnvironment.IS_REACT_ACT_ENVIRONMENT;
});

describe("GIF manual web AI handoff", () => {
  it("keeps NovelAI Undesired Content locked when an old prompt copy finishes after request and provider changes", async () => {
    actEnvironment.IS_REACT_ACT_ENVIRONMENT = true;
    const host = document.createElement("div");
    document.body.append(host);
    const root = createRoot(host);
    const clipboardDescriptor = Object.getOwnPropertyDescriptor(
      navigator,
      "clipboard",
    );
    const pendingCopy = deferredVoid();
    const writeText = vi.fn(() => pendingCopy.promise);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });

    try {
      await act(async () => {
        root.render(
          <GifAiWebExportActions
            novelAiPrompt="animated emoticon, sprite sheet"
            prompt="Gemini structure prompt"
            onContinueToReimport={() => undefined}
            onOpenAiSite={async () => undefined}
          />,
        );
      });

      const service = host.querySelector<HTMLSelectElement>(
        '[data-testid="gif-ai-web-service"]',
      );
      const desiredEdit = host.querySelector<HTMLTextAreaElement>(
        '[data-testid="gif-ai-desired-edit"]',
      );
      const copyPrompt = host.querySelector<HTMLButtonElement>(
        '[data-testid="gif-ai-copy-prompt"]',
      );
      if (!service || !desiredEdit || !copyPrompt) {
        throw new Error("Missing GIF NovelAI controls");
      }
      await act(async () => setControlValue(service, "novelai_app"));

      const undesired = () =>
        host.querySelector<HTMLButtonElement>(
          '[data-testid="novelai-copy-undesired-gif_frame_sheet"]',
        );
      expect(undesired()?.disabled).toBe(true);

      await act(async () => {
        copyPrompt.click();
        await Promise.resolve();
      });
      expect(writeText).toHaveBeenCalledTimes(1);

      await act(async () => setControlValue(desiredEdit, "different motion"));
      await act(async () => setControlValue(service, "gemini_ai_studio"));
      await act(async () => setControlValue(service, "novelai_app"));
      expect(undesired()?.disabled).toBe(true);

      await act(async () => {
        pendingCopy.resolve();
        await pendingCopy.promise;
        await Promise.resolve();
      });

      expect(undesired()?.disabled).toBe(true);
      expect(
        host.querySelector(
          '[data-testid="novelai-copy-state-gif_frame_sheet"]',
        )?.textContent,
      ).toContain("현재 1/2");
    } finally {
      pendingCopy.resolve();
      act(() => root.unmount());
      host.remove();
      if (clipboardDescriptor) {
        Object.defineProperty(navigator, "clipboard", clipboardDescriptor);
      } else {
        Reflect.deleteProperty(navigator, "clipboard");
      }
    }
  });

  it("disables provider and request controls during delayed GIF prompt copy and site opening", async () => {
    actEnvironment.IS_REACT_ACT_ENVIRONMENT = true;
    const host = document.createElement("div");
    document.body.append(host);
    const root = createRoot(host);
    const clipboardDescriptor = Object.getOwnPropertyDescriptor(
      navigator,
      "clipboard",
    );
    const pendingCopy = deferredVoid();
    const pendingOpen = deferredVoid();
    const writeText = vi.fn(() => pendingCopy.promise);
    const openAiSite = vi.fn(() => pendingOpen.promise);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });

    try {
      await act(async () => {
        root.render(
          <GifAiWebExportActions
            novelAiPrompt="animated emoticon, sprite sheet"
            prompt="Gemini structure prompt"
            onContinueToReimport={() => undefined}
            onOpenAiSite={openAiSite}
          />,
        );
      });

      const service = host.querySelector<HTMLSelectElement>(
        '[data-testid="gif-ai-web-service"]',
      );
      const desiredEdit = host.querySelector<HTMLTextAreaElement>(
        '[data-testid="gif-ai-desired-edit"]',
      );
      const copyPrompt = host.querySelector<HTMLButtonElement>(
        '[data-testid="gif-ai-copy-prompt"]',
      );
      const openSelected = host.querySelector<HTMLButtonElement>(
        '[data-testid="gif-ai-open-selected"]',
      );
      const continueReimport = host.querySelector<HTMLButtonElement>(
        '[data-testid="gif-ai-continue-reimport"]',
      );
      if (
        !service ||
        !desiredEdit ||
        !copyPrompt ||
        !openSelected ||
        !continueReimport
      ) {
        throw new Error("Missing GIF async controls");
      }
      await act(async () => {
        setControlValue(service, "novelai_app");
        setControlValue(desiredEdit, "wavy motion");
      });

      const undesired = () =>
        host.querySelector<HTMLButtonElement>(
          '[data-testid="novelai-copy-undesired-gif_frame_sheet"]',
        );
      await act(async () => {
        openSelected.click();
        await Promise.resolve();
      });

      expect(writeText).toHaveBeenCalledTimes(1);
      expect(service.disabled).toBe(true);
      expect(desiredEdit.disabled).toBe(true);
      expect(copyPrompt.disabled).toBe(true);
      expect(openSelected.disabled).toBe(true);
      expect(continueReimport.disabled).toBe(true);
      expect(undesired()?.disabled).toBe(true);

      await act(async () => {
        pendingCopy.resolve();
        await pendingCopy.promise;
        await Promise.resolve();
      });

      expect(openAiSite).toHaveBeenCalledWith("novelai_app");
      expect(service.disabled).toBe(true);
      expect(desiredEdit.disabled).toBe(true);
      expect(undesired()?.disabled).toBe(true);

      await act(async () => {
        pendingOpen.resolve();
        await pendingOpen.promise;
        await Promise.resolve();
      });

      expect(service.disabled).toBe(false);
      expect(desiredEdit.disabled).toBe(false);
      expect(copyPrompt.disabled).toBe(false);
      expect(openSelected.disabled).toBe(false);
      expect(continueReimport.disabled).toBe(false);
      expect(undesired()?.disabled).toBe(false);
    } finally {
      pendingCopy.resolve();
      pendingOpen.resolve();
      act(() => root.unmount());
      host.remove();
      if (clipboardDescriptor) {
        Object.defineProperty(navigator, "clipboard", clipboardDescriptor);
      } else {
        Reflect.deleteProperty(navigator, "clipboard");
      }
    }
  });

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
        '[data-testid="gif-ai-open-selected"]',
      );
      expect(button).not.toBeNull();
      await act(async () => {
        button?.click();
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(writeText).toHaveBeenCalledWith("PNG만 반환하세요.");
      expect(openAiSite).toHaveBeenCalledWith("gemini_ai_studio");
      expect(continueToReimport).not.toHaveBeenCalled();
      expect(host.textContent).toContain("현재 frames_sheet 페이지부터 처리");

      const service = host.querySelector<HTMLSelectElement>(
        '[data-testid="gif-ai-web-service"]',
      );
      if (!service) throw new Error("Missing web service selector");
      await act(async () => {
        const descriptor = Object.getOwnPropertyDescriptor(
          Object.getPrototypeOf(service),
          "value",
        );
        descriptor?.set?.call(service, "novelai_app");
        service.dispatchEvent(new Event("change", { bubbles: true }));
      });
      expect(host.textContent).not.toContain(
        "Gemini AI Studio 공식 사이트를 열었습니다",
      );

      await act(async () => {
        host
          .querySelector<HTMLButtonElement>(
            '[data-testid="gif-ai-continue-reimport"]',
          )
          ?.click();
      });
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

  it("uses NovelAI tags, shows Image2Image guidance, and opens NovelAI", async () => {
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
            expectedCanvas="1024×1024px"
            novelAiPrompt={"animated emoticon, frame sequence, sprite sheet\nKeep the original layout unchanged."}
            prompt="Gemini structure prompt"
            onContinueToReimport={continueToReimport}
            onOpenAiSite={openAiSite}
          />,
        );
      });

      const service = host.querySelector<HTMLSelectElement>(
        '[data-testid="gif-ai-web-service"]',
      );
      const desiredEdit = host.querySelector<HTMLTextAreaElement>(
        '[data-testid="gif-ai-desired-edit"]',
      );
      if (!service || !desiredEdit) throw new Error("Missing NovelAI controls");
      await act(async () => {
        const selectDescriptor = Object.getOwnPropertyDescriptor(
          Object.getPrototypeOf(service),
          "value",
        );
        selectDescriptor?.set?.call(service, "novelai_app");
        service.dispatchEvent(new Event("change", { bubbles: true }));
        const textareaDescriptor = Object.getOwnPropertyDescriptor(
          Object.getPrototypeOf(desiredEdit),
          "value",
        );
        textareaDescriptor?.set?.call(desiredEdit, "Wavy MOTION; shifting COLORS");
        desiredEdit.dispatchEvent(new Event("input", { bubbles: true }));
      });

      expect(host.textContent).toContain("Image2Image");
      expect(host.textContent).toContain("NovelAI 웹 호환 GIF");
      expect(
        host.querySelector<HTMLButtonElement>(
          '[data-testid="novelai-copy-undesired-gif_frame_sheet"]',
        )?.disabled,
      ).toBe(true);
      const button = host.querySelector<HTMLButtonElement>(
        '[data-testid="gif-ai-open-selected"]',
      );
      await act(async () => {
        button?.click();
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(writeText).toHaveBeenCalledWith(
        "animated emoticon, frame sequence, sprite sheet, wavy motion, shifting colors\nKeep the original layout unchanged.",
      );
      expect(openAiSite).toHaveBeenCalledWith("novelai_app");
      expect(continueToReimport).not.toHaveBeenCalled();
      expect(host.textContent).toContain("Prompt와 Undesired Content를 붙여넣고");
      expect(
        host.querySelector('[data-testid="novelai-copy-state-gif_frame_sheet"]')?.textContent,
      ).toContain("1/2 완료");
      expect(
        host.querySelector<HTMLButtonElement>(
          '[data-testid="novelai-copy-undesired-gif_frame_sheet"]',
        )?.disabled,
      ).toBe(false);

      await act(async () => {
        const descriptor = Object.getOwnPropertyDescriptor(
          Object.getPrototypeOf(desiredEdit),
          "value",
        );
        descriptor?.set?.call(desiredEdit, "different motion");
        desiredEdit.dispatchEvent(new Event("input", { bubbles: true }));
      });
      expect(host.textContent).not.toContain(
        "NovelAI 공식 사이트를 열었습니다",
      );
      expect(
        host.querySelector<HTMLButtonElement>(
          '[data-testid="novelai-copy-undesired-gif_frame_sheet"]',
        )?.disabled,
      ).toBe(true);

      await act(async () => {
        host
          .querySelector<HTMLButtonElement>(
            '[data-testid="gif-ai-continue-reimport"]',
          )
          ?.click();
      });
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
