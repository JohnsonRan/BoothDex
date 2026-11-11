"use client";
// Sign in and create an account, adapted from the Astryx page template login-card (templates/pages/login-card):
// its heading and description (page.tsx:104-107), large fields, a full-width primary button; inside a Dialog the
// centring frame and the card are dropped, and BoothDex has no third-party providers, so neither the divider nor
// the provider buttons are used. The heading and description are the Dialog's DialogHeader, as the dialog block
// DialogWithSubtitle (templates/blocks/DialogWithSubtitle.tsx:21-27) has them, with its close button: a form
// dialog is not dismissed from its scrim, so on a touch screen that button is the way out. Tabs switch between the two forms. Requests and fields are master's
// (account-menu.tsx): sign-in keeps the session (`keep: true`); errors from the server arrive as a toast.
// The admin gate opens it sign-in only, under its own heading (`gate`); so does a closed registration, which the
// dialog asks about each time it opens (the server refuses sign-up either way). Under the register button, the
// template's terms line (page.tsx:196-209) links the terms and the privacy policy, which open in a dialog over this
// one (legal-dialog.tsx), so the form stays as filled.

import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { Link } from "@astryxdesign/core/Link";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Tab, TabList } from "@astryxdesign/core/TabList";
import { Text } from "@astryxdesign/core/Text";
import { TextInput } from "@astryxdesign/core/TextInput";
import { useEffect, useState } from "react";
import { useLegalDialog } from "@/components/legal/legal-dialog";
import { useUi } from "@/components/providers";
import { useToast } from "@/components/toasts";
import { api, errorKey } from "@/lib/api";
import { API_ERRORS, type MessageKey } from "@/lib/messages";
import { rich } from "@/lib/rich";

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
  const legal = useLegalDialog();
  const [mode, setMode] = useState<Mode>("login");
  const [values, setValues] = useState<Partial<Record<Field, string>>>({});
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  const [canRegister, setCanRegister] = useState(true);
  const value = (f: Field) => values[f] ?? "";

  useEffect(() => {
    if (!isOpen || gate) return;
    api<{ open: boolean }>("/api/account/registration").then(
      ({ open }) => {
        setCanRegister(open);
        if (!open) setMode("login");
      },
      // unknown: keep offering it
      () => setCanRegister(true),
    );
  }, [isOpen, gate]);
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
        body: t(errorKey(err, API_ERRORS, "error.failed")),
        uniqueID: "auth",
      });
    } finally {
      setBusy(false);
    }
  };

  const label = mode === "login" ? t("account.signIn") : t("account.register");
  return (
    <>
    <Dialog isOpen={isOpen} onOpenChange={onOpenChange} purpose="form">
      <Layout
        header={
          <DialogHeader
            title={gate?.title ?? t("account.title")}
            subtitle={gate?.description ?? t("account.desc")}
            onOpenChange={onOpenChange}
          />
        }
        content={
          <LayoutContent>
            <VStack gap={4} hAlign="stretch">
              {!gate && canRegister && (
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
                          // the rule stays readable while typing (template form-wizard, page.tsx:453)
                          description={
                            f === "username" && mode === "register"
                              ? t("account.usernameRule")
                              : undefined
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
                            error
                              ? { type: "error", message: t(error) }
                              : undefined
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
                  {mode === "register" && (
                    <Text type="supporting" color="secondary" justify="center">
                      {rich(t("legal.consent"), {
                        terms: (
                          <Link
                            href="/terms"
                            type="supporting"
                            onClick={legal.open("terms")}
                          >
                            {t("legal.terms")}
                          </Link>
                        ),
                        privacy: (
                          <Link
                            href="/privacy"
                            type="supporting"
                            onClick={legal.open("privacy")}
                          >
                            {t("legal.privacy")}
                          </Link>
                        ),
                      })}
                    </Text>
                  )}
                </VStack>
              </form>
            </VStack>
          </LayoutContent>
        }
      />
    </Dialog>
    {legal.dialog}
    </>
  );
}
