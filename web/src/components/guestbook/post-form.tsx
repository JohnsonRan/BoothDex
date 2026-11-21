"use client";
// The guestbook's form, from the Astryx dialog block DrawerForm's fields: nickname, message with its counter, the
// Turnstile widget and Post. A message is final once posted, so Post first shows it back in a confirm dialog
// (DialogWithSubtitle's header, the text in the content, Back and the action in the footer); only the confirm sends
// it. The widget is reset after every attempt, since Cloudflare takes each token once. A token that expired while the
// confirm dialog stayed open is not sent: the confirm waits for the fresh one the widget hands out by itself, and
// without one it closes and asks for the check again.

import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import {
  HStack,
  Layout,
  LayoutContent,
  LayoutFooter,
  VStack,
} from "@astryxdesign/core/Layout";
import { Text } from "@astryxdesign/core/Text";
import { TextArea } from "@astryxdesign/core/TextArea";
import { TextInput } from "@astryxdesign/core/TextInput";
import { useEffect, useRef, useState } from "react";
import { useToast } from "@/components/toasts";
import {
  useVerificationT,
  verificationRefused,
} from "@/components/verification/messages";
import { VerificationField } from "@/components/verification/verification-field";
import { api } from "@/lib/api";
import type { GuestbookEntry } from "@/lib/api-types";
import { useTurnstile } from "@/lib/turnstile";
import { MESSAGE_STYLE } from "./message-style";
import { guestbookError, useGuestbookT } from "./messages";
import {
  BODY_LINES,
  BODY_MAX,
  bodyLength,
  bodyOk,
  NICKNAME_MAX,
  nicknameOk,
  normalBody,
} from "./rules";

/** the action the API checks the token for (crates/api/src/guestbook.rs ACTION) */
const ACTION = "guestbook";

/** How long a confirm waits for the widget to replace an expired token. */
export const FRESH_TOKEN_WAIT_MS = 5000;

/**
 * The token a confirm sends: the one the widget holds, or, when it expired, the next one the widget hands out within
 * `ms` (in managed mode it replaces an expired token by itself); "" when none comes. `next` hands each new token to
 * its argument and returns how to stop.
 */
export function tokenToSend(
  held: string,
  next: (give: (token: string) => void) => () => void,
  ms = FRESH_TOKEN_WAIT_MS,
): Promise<string> {
  if (held) return Promise.resolve(held);
  return new Promise((resolve) => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const done = (token: string) => {
      clearTimeout(timer);
      stop();
      resolve(token);
    };
    const stop = next(done);
    timer = setTimeout(() => done(""), ms);
  });
}

export function PostForm({
  siteKey,
  onPosted,
  onClosed,
}: {
  siteKey: string;
  onPosted: (entry: GuestbookEntry) => void;
  /** the API answered that the guestbook is not open (an admin cleared a Turnstile key meanwhile) */
  onClosed: () => void;
}) {
  const t = useGuestbookT();
  const vt = useVerificationT();
  const toast = useToast();
  const verification = useTurnstile(siteKey, ACTION);
  const [nickname, setNickname] = useState("");
  const [body, setBody] = useState("");
  const [checked, setChecked] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  // the widget's latest token, and the confirm waiting for a fresh one
  const latest = useRef("");
  const waiting = useRef<((token: string) => void) | null>(null);
  useEffect(() => {
    latest.current = verification.token;
    if (verification.token) waiting.current?.(verification.token);
  }, [verification.token]);
  const nicknameBad = checked && !nicknameOk(nickname);
  const length = bodyLength(body);
  // a message over the limit shows it at once, as a counter turning red would
  const bodyBad = (checked && !bodyOk(body)) || length > BODY_MAX;

  const review = () => {
    setChecked(true);
    if (!nicknameOk(nickname) || !bodyOk(body)) return;
    if (!verification.token) {
      toast({ type: "error", body: vt("required"), uniqueID: "guestbook" });
      return;
    }
    setConfirming(true);
  };
  const post = async () => {
    setBusy(true);
    const token = await tokenToSend(latest.current, (give) => {
      waiting.current = give;
      return () => {
        waiting.current = null;
      };
    });
    if (!token) {
      setConfirming(false);
      setBusy(false);
      toast({ type: "error", body: vt("required"), uniqueID: "guestbook" });
      return;
    }
    try {
      const entry = await api<GuestbookEntry>("/api/guestbook", {
        method: "POST",
        json: { nickname, body, token },
      });
      setBody("");
      setChecked(false);
      setConfirming(false);
      onPosted(entry);
      toast({ body: t("posted"), uniqueID: "guestbook" });
    } catch (e) {
      setConfirming(false);
      const why = guestbookError(e);
      toast({
        type: "error",
        body: verificationRefused(e) ? vt("refused") : t(why),
        uniqueID: "guestbook",
      });
      if (why === "closed") onClosed();
    } finally {
      verification.reset();
      setBusy(false);
    }
  };

  return (
    <VStack gap={4}>
      <TextInput
        label={t("nickname")}
        autoComplete="nickname"
        status={
          nicknameBad
            ? {
                type: "error",
                message: t("nicknameRule", { max: NICKNAME_MAX }),
              }
            : undefined
        }
        value={nickname}
        onChange={setNickname}
      />
      {/* the count in the description, not TextArea's maxLength counter: that one counts what a reader sees as one
          character (an emoji sequence, a letter with its accent), the server counts Unicode scalar values */}
      <TextArea
        label={t("body")}
        description={t("bodyCount", { n: length, max: BODY_MAX })}
        autoComplete="off"
        rows={5}
        status={
          bodyBad
            ? {
                type: "error",
                message: t("bodyRule", { max: BODY_MAX, lines: BODY_LINES }),
              }
            : undefined
        }
        value={body}
        onChange={setBody}
      />
      <VerificationField verification={verification} />
      <HStack hAlign="end">
        <Button
          label={t("post")}
          variant="primary"
          isLoading={busy}
          onClick={review}
        />
      </HStack>
      <Dialog
        isOpen={confirming}
        onOpenChange={(open) => !busy && setConfirming(open)}
        purpose="form"
        width={480}
      >
        <Layout
          header={
            <DialogHeader
              title={t("confirmTitle")}
              subtitle={t("confirmDesc")}
              onOpenChange={(open) => !busy && setConfirming(open)}
            />
          }
          content={
            <LayoutContent>
              <VStack gap={1}>
                <Text type="label">{nickname.trim()}</Text>
                <Text type="body" as="p" style={MESSAGE_STYLE}>
                  {normalBody(body)}
                </Text>
              </VStack>
            </LayoutContent>
          }
          footer={
            <LayoutFooter>
              <HStack gap={2} hAlign="end">
                <Button
                  label={t("back")}
                  variant="secondary"
                  isDisabled={busy}
                  onClick={() => setConfirming(false)}
                />
                <Button
                  label={t("confirm")}
                  variant="primary"
                  isLoading={busy}
                  onClick={post}
                />
              </HStack>
            </LayoutFooter>
          }
        />
      </Dialog>
    </VStack>
  );
}
