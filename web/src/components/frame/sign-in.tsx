"use client";
// Sign in and create an account, adapted from the Astryx page template login-card (templates/pages/login-card):
// its heading and description, large fields, a full-width primary button; inside a Dialog the centring frame
// and the card are dropped, and BoothDex has no third-party providers, so neither the divider nor the provider
// buttons are used. Tabs switch between the two forms. Requests and fields are master's
// (account-menu.tsx): sign-in keeps the session (`keep: true`); errors from the server arrive as a toast.
// The admin gate opens it sign-in only, under its own heading (`gate`).

import { Button } from "@astryxdesign/core/Button";
import { Dialog } from "@astryxdesign/core/Dialog";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Tab, TabList } from "@astryxdesign/core/TabList";
import { Heading, Text } from "@astryxdesign/core/Text";
import { TextInput } from "@astryxdesign/core/TextInput";
import { useToast } from "@astryxdesign/core/Toast";
import { useState } from "react";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import type { MessageKey } from "@/lib/messages";

type Mode = "login" | "register";
type Field = "login" | "password" | "display_name" | "username" | "email";
const FIELDS: Record<Mode, Field[]> = {
  login: ["login", "password"],
  register: ["display_name", "username", "email", "password"],
};

/** The first rule a value breaks, as a message key; master enforced the same through input attributes. */
function problem(mode: Mode, field: Field, v: string): MessageKey | undefined {
  if (!v.trim()) return "account.required";
  if (mode === "login") return undefined;
  if (field === "username" && !/^[A-Za-z0-9_]{3,24}$/.test(v))
    return "account.usernameRule";
  if (field === "email" && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v))
    return "account.emailInvalid";
  if (field === "password" && v.length < 10) return "account.passwordShort";
}

const INPUT: Record<
  Field,
  { label: MessageKey; type?: "password" | "email"; auto?: string }
> = {
  login: { label: "account.login", auto: "username" },
  password: { label: "account.password", type: "password" },
  display_name: { label: "account.displayName", auto: "nickname" },
  username: { label: "account.username", auto: "username" },
  email: { label: "account.email", type: "email", auto: "email" },
};

export function SignInDialog({
  isOpen,
  onOpenChange,
  onSignedIn,
  gate,
}: {
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
  onSignedIn: () => void;
  /** sign-in only, under this heading (the admin gate); otherwise sign in or register under the site's */
  gate?: { title: string; description: string };
}) {
  const { t } = useUi();
  const toast = useToast();
  const [mode, setMode] = useState<Mode>("login");
  const [values, setValues] = useState<Partial<Record<Field, string>>>({});
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  const value = (f: Field) => values[f] ?? "";
  const errorOf = (f: Field) =>
    checked ? problem(mode, f, value(f)) : undefined;

  const submit = async () => {
    setChecked(true);
    if (FIELDS[mode].some((f) => problem(mode, f, value(f)))) return;
    setBusy(true);
    try {
      await api(
        mode === "login" ? "/api/account/login" : "/api/account/register",
        {
          method: "POST",
          json: {
            ...Object.fromEntries(FIELDS[mode].map((f) => [f, value(f)])),
            keep: true,
          },
        },
      );
      toast({ body: t("account.signedIn"), uniqueID: "auth" });
      setValues({});
      setChecked(false);
      onSignedIn();
    } catch (err) {
      toast({
        type: "error",
        body: err instanceof Error ? err.message : String(err),
        uniqueID: "auth",
      });
    } finally {
      setBusy(false);
    }
  };

  const label = mode === "login" ? t("account.signIn") : t("account.register");
  return (
    <Dialog
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      purpose="form"
      padding={8}
    >
      <VStack gap={4} hAlign="stretch">
        <VStack gap={1} hAlign="center">
          <Heading level={2}>{gate?.title ?? t("account.title")}</Heading>
          <Text type="body" color="secondary" size="sm">
            {gate?.description ?? t("account.desc")}
          </Text>
        </VStack>
        {!gate && (
          <TabList
            value={mode}
            onChange={(v) => {
              setMode(v as Mode);
              setChecked(false);
            }}
            layout="fill"
            role="tablist"
          >
            <Tab value="login" label={t("account.signIn")} />
            <Tab value="register" label={t("account.register")} />
          </TabList>
        )}
        {/* a form, so Enter submits and password managers recognise it */}
        <form
          noValidate
          onSubmit={(e) => {
            e.preventDefault();
            submit();
          }}
        >
          <VStack gap={4} hAlign="stretch">
            <VStack gap={2}>
              {FIELDS[mode].map((f) => {
                const { label: name, type, auto } = INPUT[f];
                const error = errorOf(f);
                return (
                  <TextInput
                    key={`${mode}-${f}`}
                    label={t(name)}
                    isLabelHidden
                    placeholder={
                      f === "password" && mode === "register"
                        ? t("account.newPassword")
                        : t(name)
                    }
                    type={type}
                    autoComplete={
                      f === "password"
                        ? mode === "login"
                          ? "current-password"
                          : "new-password"
                        : auto
                    }
                    value={value(f)}
                    onChange={(v: string) =>
                      setValues((s) => ({ ...s, [f]: v }))
                    }
                    size="lg"
                    status={
                      error ? { type: "error", message: t(error) } : undefined
                    }
                  />
                );
              })}
            </VStack>
            <HStack>
              <Button
                type="submit"
                label={label}
                variant="primary"
                size="lg"
                width="100%"
                isLoading={busy}
              />
            </HStack>
          </VStack>
        </form>
      </VStack>
    </Dialog>
  );
}
