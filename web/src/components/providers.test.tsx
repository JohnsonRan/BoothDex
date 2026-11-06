import { afterEach, expect, mock, test } from "bun:test";
import {
  AppRouterContext,
  type AppRouterInstance,
} from "next/dist/shared/lib/app-router-context.shared-runtime";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MESSAGES } from "@/lib/dictionaries";
import { DEFAULT_SETTINGS, SETTINGS_COOKIE } from "@/lib/settings-core";

// the server renderer has no transitions; run the refresh at once
mock.module("react", () => ({
  ...React,
  useTransition: () => [false, (run: () => void) => run()],
}));
const { Providers, useUi } = await import("./providers");

const realDocument = globalThis.document;
afterEach(() => {
  globalThis.document = realDocument;
});

let update: ReturnType<typeof useUi>["update"] = async () => true;
function Grab() {
  update = useUi().update;
  return null;
}

test("a second change made before the page renders again keeps the first", () => {
  renderToStaticMarkup(
    <AppRouterContext value={{ refresh() {} } as AppRouterInstance}>
      <Providers settings={DEFAULT_SETTINGS} messages={MESSAGES.zh}>
        <Grab />
      </Providers>
    </AppRouterContext>,
  );
  let cookie = "";
  globalThis.document = {
    documentElement: {},
    set cookie(v: string) {
      cookie = v;
    },
  } as Document;
  update({ ui: "en" });
  update({ currency: "USD" });
  const saved = JSON.parse(
    decodeURIComponent(
      cookie.slice(`${SETTINGS_COOKIE}=`.length).split(";")[0],
    ),
  );
  expect(saved).toEqual({ ...DEFAULT_SETTINGS, ui: "en", currency: "USD" });
});
