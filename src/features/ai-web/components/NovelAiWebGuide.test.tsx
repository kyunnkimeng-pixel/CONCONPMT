// @vitest-environment happy-dom

import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";

import { NovelAiWebGuide } from "@/features/ai-web/components/NovelAiWebGuide";

const actEnvironment = globalThis as typeof globalThis & {
  IS_REACT_ACT_ENVIRONMENT?: boolean;
};

afterEach(() => {
  delete actEnvironment.IS_REACT_ACT_ENVIRONMENT;
  vi.restoreAllMocks();
});

describe("NovelAiWebGuide", () => {
  it("shows the mode, exact resolution rule, and copies Undesired Content separately", async () => {
    actEnvironment.IS_REACT_ACT_ENVIRONMENT = true;
    const host = document.createElement("div");
    document.body.append(host);
    const root = createRoot(host);
    const writeText = vi.fn().mockResolvedValue(undefined);
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
          <NovelAiWebGuide
            expectedCanvas="1024×1024px"
            promptCopyOutcome="idle"
            promptCopyRevision={0}
            task="gif_frame_sheet"
          />,
        );
      });

      expect(host.textContent).toContain("Image2Image");
      expect(host.textContent).toContain("Add a Base Img (Optional)");
      expect(host.textContent).toContain("What do you want to do with this image?");
      expect(host.textContent).toContain("1024×1024px가 정확히 유지");
      expect(host.textContent).toContain("복사 순서: Prompt → Undesired Content");
      expect(host.textContent).toContain("현재 1/2");
      expect(host.textContent).toContain("Undesired Content 입력란");
      expect(host.textContent).toContain("아래쪽 별도 필드");
      expect(host.textContent).toContain("Account Settings를 연 뒤 Image Settings 탭 → Image Generation → Image Format for Generated Images");
      const button = host.querySelector<HTMLButtonElement>(
        '[data-testid="novelai-copy-undesired-gif_frame_sheet"]',
      );
      expect(button?.disabled).toBe(true);
      await act(async () => {
        root.render(
          <NovelAiWebGuide
            expectedCanvas="1024×1024px"
            promptCopyOutcome="copied"
            promptCopyRevision={1}
            task="gif_frame_sheet"
          />,
        );
      });
      expect(button?.disabled).toBe(false);
      expect(
        host.querySelector('[data-testid="novelai-copy-state-gif_frame_sheet"]')?.textContent,
      ).toContain("1/2 완료");
      await act(async () => {
        button?.click();
        await Promise.resolve();
      });
      expect(writeText).toHaveBeenCalledWith(
        expect.stringContaining("merged cells"),
      );
      expect(
        host.querySelector('[data-testid="novelai-copy-state-gif_frame_sheet"]')?.textContent,
      ).toContain("2/2 완료");
      expect(host.textContent).toContain("Undesired Content가 복사되었습니다");
      await act(async () => {
        root.render(
          <NovelAiWebGuide
            expectedCanvas="1024×1024px"
            promptCopyOutcome="copied"
            promptCopyRevision={2}
            task="gif_frame_sheet"
          />,
        );
      });
      expect(
        host.querySelector('[data-testid="novelai-copy-state-gif_frame_sheet"]')?.textContent,
      ).toContain("1/2 완료");
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

  it("keeps checker warnings but removes opaque and forced background removal in allow mode", async () => {
    actEnvironment.IS_REACT_ACT_ENVIRONMENT = true;
    const host = document.createElement("div");
    document.body.append(host);
    const root = createRoot(host);

    try {
      await act(async () => {
        root.render(
          <NovelAiWebGuide
            backgroundPolicy="allow_opaque"
            expectedCanvas="1024×1024px"
            task="grid_generate"
          />,
        );
      });

      const undesired = host.querySelector<HTMLTextAreaElement>(
        '[data-testid="novelai-undesired-grid_generate"]',
      );
      expect(undesired?.value).toContain("checkerboard");
      expect(undesired?.value).toContain("fake transparency");
      expect(undesired?.value).not.toContain("opaque background");
      expect(host.textContent).toContain("배경 제거 도구가 필수가 아닙니다");
      expect(host.textContent).not.toContain("Remove BG");
    } finally {
      act(() => root.unmount());
      host.remove();
    }
  });

  it("announces an Undesired Content copy failure as an actionable alert", async () => {
    actEnvironment.IS_REACT_ACT_ENVIRONMENT = true;
    const host = document.createElement("div");
    document.body.append(host);
    const root = createRoot(host);
    const clipboardDescriptor = Object.getOwnPropertyDescriptor(
      navigator,
      "clipboard",
    );
    const execDescriptor = Object.getOwnPropertyDescriptor(
      document,
      "execCommand",
    );
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: vi.fn().mockRejectedValue(new Error("denied")) },
    });
    Object.defineProperty(document, "execCommand", {
      configurable: true,
      value: vi.fn(() => false),
    });

    try {
      await act(async () => {
        root.render(
          <NovelAiWebGuide expectedCanvas="200×200px" task="single_edit" />,
        );
      });
      await act(async () => {
        host
          .querySelector<HTMLButtonElement>(
            '[data-testid="novelai-copy-undesired-single_edit"]',
          )
          ?.click();
        await Promise.resolve();
        await Promise.resolve();
      });

      const state = host.querySelector(
        '[data-testid="novelai-copy-state-single_edit"]',
      );
      expect(state?.getAttribute("role")).toBe("alert");
      expect(state?.textContent).toContain("직접 복사");
    } finally {
      act(() => root.unmount());
      host.remove();
      if (clipboardDescriptor) {
        Object.defineProperty(navigator, "clipboard", clipboardDescriptor);
      } else {
        Reflect.deleteProperty(navigator, "clipboard");
      }
      if (execDescriptor) {
        Object.defineProperty(document, "execCommand", execDescriptor);
      } else {
        Reflect.deleteProperty(document, "execCommand");
      }
    }
  });
});
