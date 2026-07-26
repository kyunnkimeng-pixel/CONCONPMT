export const ADVANCED_EDITOR_MODES = [
  "effects",
  "motion",
  "tools",
] as const;

export type AdvancedEditorMode = (typeof ADVANCED_EDITOR_MODES)[number];

export function nextAdvancedEditorMode(
  current: AdvancedEditorMode,
  key: string,
): AdvancedEditorMode | null {
  const currentIndex = ADVANCED_EDITOR_MODES.indexOf(current);

  switch (key) {
    case "ArrowRight":
      return ADVANCED_EDITOR_MODES[
        (currentIndex + 1) % ADVANCED_EDITOR_MODES.length
      ];
    case "ArrowLeft":
      return ADVANCED_EDITOR_MODES[
        (currentIndex - 1 + ADVANCED_EDITOR_MODES.length) %
          ADVANCED_EDITOR_MODES.length
      ];
    case "Home":
      return ADVANCED_EDITOR_MODES[0];
    case "End":
      return ADVANCED_EDITOR_MODES[ADVANCED_EDITOR_MODES.length - 1];
    default:
      return null;
  }
}
