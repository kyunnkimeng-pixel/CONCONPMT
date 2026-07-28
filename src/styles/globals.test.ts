import { readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";

const globalsCss = readFileSync(
  path.resolve(process.cwd(), "src/styles/globals.css"),
  "utf8",
);

describe("global reduced-motion contract", () => {
  it("removes AI workspace animation and transition without affecting other surfaces", () => {
    expect(globalsCss).toMatch(
      /@media\s*\(prefers-reduced-motion:\s*reduce\)/,
    );
    expect(globalsCss).toContain('[data-testid="ai-workspace-overlay"] *');
    expect(globalsCss).toContain("animation: none !important;");
    expect(globalsCss).toContain("scroll-behavior: auto !important;");
    expect(globalsCss).toContain("transition: none !important;");
  });
});