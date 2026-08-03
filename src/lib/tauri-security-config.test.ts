import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

interface TauriSecurityConfig {
  identifier: string;
  app: {
    security: {
      assetProtocol: {
        enable: boolean;
        scope: string[];
      };
      csp: Record<string, string>;
      devCsp: Record<string, string>;
    };
  };
}

function readTauriConfig() {
  const configUrl = new URL("../../src-tauri/tauri.conf.json", import.meta.url);
  return JSON.parse(
    readFileSync(configUrl, "utf8"),
  ) as TauriSecurityConfig;
}

describe("Tauri asset protocol security config", () => {
  it("allows the complete application-specific AppData directory exactly once", () => {
    const config = readTauriConfig();

    expect(config.identifier).toBe("com.pmtconcon.studio");
    expect(config.app.security.assetProtocol).toEqual({
      enable: true,
      scope: ["$APPDATA/**"],
    });
    expect(config.app.security.assetProtocol.scope).not.toContain(
      `$APPDATA/${config.identifier}/**`,
    );
  });

  it("allows Tauri asset URLs in production and development image CSP", () => {
    const { security } = readTauriConfig().app;

    for (const csp of [security.csp, security.devCsp]) {
      expect(csp["img-src"].split(/\s+/)).toEqual(
        expect.arrayContaining(["asset:", "http://asset.localhost"]),
      );
    }
  });
});
