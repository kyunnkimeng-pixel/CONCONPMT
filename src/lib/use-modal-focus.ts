import { useCallback, useEffect, useRef } from "react";
import type { RefObject } from "react";

const FOCUSABLE_SELECTOR = [
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "a[href]",
  '[contenteditable]:not([contenteditable="false"])',
  '[tabindex]:not([tabindex="-1"])',
].join(",");

const activeModalFocusTokens: symbol[] = [];

export function isTopmostModalFocusToken(
  activeTokens: readonly symbol[],
  token: symbol,
) {
  return activeTokens[activeTokens.length - 1] === token;
}

export interface UseModalFocusOptions {
  /**
   * Keeps the historical behavior by default. Set this to false for a modal
   * that always hands focus to another surface when it closes.
   */
  restoreFocusOnClose?: boolean;
  /** Resolves the preferred target inside the mounted dialog before fallback focus. */
  initialFocus?: (dialog: HTMLElement) => HTMLElement | null;
  /** Runs after the initial target or fallback receives focus. */
  onInitialFocusApplied?: () => void;
}

export interface ModalFocusHandle {
  /**
   * Call immediately before an external reveal/open handoff closes the modal.
   * This prevents cleanup from moving focus back to the original trigger.
   */
  suppressFocusRestore: () => void;
  /** Re-enables normal trigger restoration if a planned handoff is cancelled. */
  resumeFocusRestore: () => void;
}

export function shouldRestoreModalFocus(
  restoreFocusOnClose: boolean,
  focusRestoreSuppressed: boolean,
) {
  return restoreFocusOnClose && !focusRestoreSuppressed;
}

export function useModalFocus(
  dialogRef: RefObject<HTMLElement | null>,
  onClose: () => void,
  options: UseModalFocusOptions = {},
): ModalFocusHandle {
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const restoreFocusOnCloseRef = useRef(options.restoreFocusOnClose ?? true);
  restoreFocusOnCloseRef.current = options.restoreFocusOnClose ?? true;
  const initialFocusRef = useRef(options.initialFocus);
  initialFocusRef.current = options.initialFocus;
  const onInitialFocusAppliedRef = useRef(options.onInitialFocusApplied);
  onInitialFocusAppliedRef.current = options.onInitialFocusApplied;
  const focusRestoreSuppressedRef = useRef(false);

  const suppressFocusRestore = useCallback(() => {
    focusRestoreSuppressedRef.current = true;
  }, []);
  const resumeFocusRestore = useCallback(() => {
    focusRestoreSuppressedRef.current = false;
  }, []);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) {
      return;
    }

    const modalFocusToken = Symbol("modal-focus");
    activeModalFocusTokens.push(modalFocusToken);
    focusRestoreSuppressedRef.current = false;
    const previousFocus = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    const animationFrame = window.requestAnimationFrame(() => {
      const requestedFocusTarget = initialFocusRef.current?.(dialog) ?? null;
      const autoFocusTarget = dialog.querySelector<HTMLElement>("[autofocus]");
      (requestedFocusTarget ?? autoFocusTarget ?? focusableElements(dialog)[0] ?? dialog).focus();
      onInitialFocusAppliedRef.current?.();
    });

    const handleKeyDown = (event: KeyboardEvent) => {
      if (!isTopmostModalFocusToken(activeModalFocusTokens, modalFocusToken)) {
        return;
      }

      if (event.key === "Escape") {
        event.preventDefault();
        onCloseRef.current();
        return;
      }

      if (event.key !== "Tab") {
        return;
      }

      const elements = focusableElements(dialog);
      if (elements.length === 0) {
        event.preventDefault();
        dialog.focus();
        return;
      }

      const first = elements[0];
      const last = elements[elements.length - 1];
      if (!dialog.contains(document.activeElement)) {
        event.preventDefault();
        (event.shiftKey ? last : first).focus();
      } else if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };

    document.addEventListener("keydown", handleKeyDown, true);
    return () => {
      window.cancelAnimationFrame(animationFrame);
      document.removeEventListener("keydown", handleKeyDown, true);
      const tokenIndex = activeModalFocusTokens.lastIndexOf(modalFocusToken);
      if (tokenIndex >= 0) {
        activeModalFocusTokens.splice(tokenIndex, 1);
      }
      if (
        previousFocus?.isConnected &&
        shouldRestoreModalFocus(
          restoreFocusOnCloseRef.current,
          focusRestoreSuppressedRef.current,
        )
      ) {
        previousFocus.focus();
      }
    };
  }, [dialogRef]);

  return { suppressFocusRestore, resumeFocusRestore };
}

function focusableElements(container: HTMLElement) {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) => element.getClientRects().length > 0,
  );
}
