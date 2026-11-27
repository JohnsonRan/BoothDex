import { expect, test } from "bun:test";
import {
  AppRouterContext,
  type AppRouterInstance,
} from "next/dist/shared/lib/app-router-context.shared-runtime";
import { renderToStaticMarkup } from "react-dom/server";
import { Providers } from "@/components/providers";
import { MESSAGES } from "@/lib/dictionaries";
import { DEFAULT_SETTINGS } from "@/lib/settings-core";
import { PasswordField } from "./password-field";

test("PasswordField exposes password semantics, toggle state, and single error message", () => {
  const html = renderToStaticMarkup(
    <AppRouterContext value={{} as AppRouterInstance}>
      <Providers
        settings={{ ...DEFAULT_SETTINGS, ui: "en" }}
        messages={MESSAGES.en}
      >
        <PasswordField
          label="Password"
          htmlName="password"
          autoComplete="current-password"
          isRequired
          value=""
          onChange={() => {}}
          status={{ type: "error", message: "Password is missing" }}
        />
      </Providers>
    </AppRouterContext>,
  );
  const labels = html
    .match(/<input [^>]*aria-labelledby="([^"]*)"/)?.[1]
    .split(" ")
    .map((id) => html.match(new RegExp(`id="${id}"[^>]*>([^<]*)`))?.[1]);
  expect(labels?.filter(Boolean)).toEqual(["Password"]);
  expect(html).toContain('type="password"');
  expect(html).toContain('name="password"');
  expect(html).toContain('autoComplete="current-password"');
  expect(html).toContain('aria-required="true"');
  expect(html).toContain('aria-invalid="true"');
  expect(html).toContain('aria-pressed="false"');
  expect(html).toContain('aria-label="Show password"');
  expect(html.match(/>Password is missing</g)).toHaveLength(1);
});
