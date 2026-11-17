"use client";
// The guestbook's form, from the Astryx dialog block DrawerForm's fields: nickname, message with its counter, the
// Turnstile widget and Post. A message is final once posted, so Post first shows it back in a confirm dialog
// (DialogWithSubtitle's header, the text in the content, Back and the action in the footer); only the confirm sends
// it. The widget is reset after every attempt, since Cloudflare takes each token once.

import { Banner } from "@astryxdesign/core/Banner";
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
import { useState } from "react";
import { useToast } from "@/components/toasts";
import { api } from "@/lib/api";
import type { GuestbookEntry } from "@/lib/api-types";
import { useTurnstile } from "@/lib/turnstile";
import { guestbookError, useGuestbookT } from "./messages";
import { BODY_MAX, bodyOk, NICKNAME_MAX, nicknameOk } from "./rules";

/** the action the API checks the token for (crates/api/src/guestbook.rs ACTION) */
const ACTION = "guestbook";

export const MESSAGE_STYLE = {
  whiteSpace: "pre-wrap",
  overflowWrap: "anywhere",
} as const;

export function PostForm({
  siteKey,
  onPosted,
}: {
  siteKey: string;
  onPosted: (entry: GuestbookEntry) => void;
}) {
  const t = useGuestbookT();
  const toast = useToast();
  const verification = useTurnstile(siteKey, ACTION);
  const [nickname, setNickname] = useState("");
  const [body, setBody] = useState("");
  const [checked, setChecked] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const nicknameBad = checked && !nicknameOk(nickname);
  const bodyBad = checked && !bodyOk(body);

  const review = () => {
    setChecked(true);
    if (!nicknameOk(nickname) || !bodyOk(body)) return;
    if (!verification.token) {
      toast({ type: "error", body: t("verify"), uniqueID: "guestbook" });
      return;
    }
    setConfirming(true);
  };
  const post = async () => {
    setBusy(true);
    try {
      const entry = await api<GuestbookEntry>("/api/guestbook", {
        method: "POST",
        json: { nickname, body, token: verification.token },
      });
      setBody("");
      setChecked(false);
      setConfirming(false);
      onPosted(entry);
      toast({ body: t("posted"), uniqueID: "guestbook" });
    } catch (e) {
      setConfirming(false);
      toast({
        type: "error",
        body: t(guestbookError(e)),
        uniqueID: "guestbook",
      });
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
        description={t("count", {
          n: [...nickname.trim()].length,
          max: NICKNAME_MAX,
        })}
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
      <TextArea
        label={t("body")}
        autoComplete="off"
        rows={5}
        maxLength={BODY_MAX}
        status={
          bodyBad
            ? { type: "error", message: t("bodyRule", { max: BODY_MAX }) }
            : undefined
        }
        value={body}
        onChange={setBody}
      />
      {verification.failed ? (
        <Banner
          status="error"
          title={t("verifyFailedLoad")}
          endContent={
            <Button
              label={t("retry")}
              variant="secondary"
              onClick={verification.retry}
            />
          }
        />
      ) : (
        <VStack ref={verification.ref} />
      )}
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
                  {body.trim()}
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
