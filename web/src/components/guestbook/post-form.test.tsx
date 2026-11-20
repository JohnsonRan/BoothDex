import { expect, test } from "bun:test";
import {
  AppRouterContext,
  type AppRouterInstance,
} from "next/dist/shared/lib/app-router-context.shared-runtime";
import { renderToStaticMarkup } from "react-dom/server";
import { Providers } from "@/components/providers";
import { MESSAGES } from "@/lib/dictionaries";
import { DEFAULT_SETTINGS } from "@/lib/settings-core";
import { PostForm } from "./post-form";

test("the message's counter counts as the server does, not TextArea's characters", () => {
  const html = renderToStaticMarkup(
    <AppRouterContext value={{} as AppRouterInstance}>
      <Providers
        settings={{ ...DEFAULT_SETTINGS, ui: "en" }}
        messages={MESSAGES.en}
      >
        <PostForm siteKey="0x-site" onPosted={() => {}} />
      </Providers>
    </AppRouterContext>,
  );
  expect(html).toContain("0 of 500 characters");
  // TextArea's own counter would show "0/500", counted by what a reader sees as one character
  expect(html).not.toContain("0/500");
});
