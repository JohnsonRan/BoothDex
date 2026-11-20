import { expect, test } from "bun:test";
import {
  AppRouterContext,
  type AppRouterInstance,
} from "next/dist/shared/lib/app-router-context.shared-runtime";
import { renderToStaticMarkup } from "react-dom/server";
import { Providers } from "@/components/providers";
import { MESSAGES } from "@/lib/dictionaries";
import { DEFAULT_SETTINGS } from "@/lib/settings-core";
import { PostForm, tokenToSend } from "./post-form";

test("the message's counter counts as the server does, not TextArea's characters", () => {
  const html = renderToStaticMarkup(
    <AppRouterContext value={{} as AppRouterInstance}>
      <Providers
        settings={{ ...DEFAULT_SETTINGS, ui: "en" }}
        messages={MESSAGES.en}
      >
        <PostForm siteKey="0x-site" onPosted={() => {}} onClosed={() => {}} />
      </Providers>
    </AppRouterContext>,
  );
  expect(html).toContain("0 of 500 characters");
  // TextArea's own counter would show "0/500", counted by what a reader sees as one character
  expect(html).not.toContain("0/500");
});

test("a confirm sends the token held, or waits for the widget to replace an expired one", async () => {
  let stopped = 0;
  const never = () => () => {
    stopped++;
  };
  expect(await tokenToSend("held", never, 20)).toBe("held");
  // expired: the widget hands out a fresh token a moment later
  const fresh = (give: (t: string) => void) => {
    const timer = setTimeout(() => give("fresh"), 5);
    return () => {
      clearTimeout(timer);
      stopped++;
    };
  };
  expect(await tokenToSend("", fresh, 200)).toBe("fresh");
  // expired and nothing comes: nothing is sent
  expect(await tokenToSend("", never, 20)).toBe("");
  expect(stopped).toBe(2);
});
