"use client";
// Sign in and create an account, adapted from the Astryx page template login-card (@astryxdesign/cli assets/templates/pages/login-card):
// its heading and description (page.tsx:104-107), large fields, a full-width primary button; inside a Dialog the
// centring frame and the card are dropped, and BoothDex has no third-party providers, so neither the divider nor
// the provider buttons are used. The heading and description are the Dialog's DialogHeader, as the dialog block
// DialogWithSubtitle (@astryxdesign/cli assets/templates/blocks/DialogWithSubtitle.tsx:21-27) has them, with its close button: a form
// dialog is not dismissed from its scrim, so on a touch screen that button is the way out. Tabs switch between the two forms. Requests and fields are master's
// (account-menu.tsx): sign-in keeps the session (`keep: true`); errors from the server arrive as a toast.
// The admin gate opens it sign-in only, under its own heading (`gate`); so does a closed registration, which the
// dialog asks about each time it opens (the server refuses sign-up either way). Under the register button, the
// template's terms line (page.tsx:196-209) links the terms and the privacy policy, which open in a dialog over this
// one (legal-dialog.tsx), so the form stays as filled.
// While the registration answer names a Turnstile site key, both forms carry the widget between the fields and the
// button, for the action the API checks ("sign-in" or "register"); a submit waits for that answer, asking once more
// when it failed, and then for the visitor to pass, unless the widget's script did not load or no answer came, when
// the server decides (it lets a request through while Cloudflare is out of reach); sign-in-gate.ts. The widget is
// reset after every attempt, since Cloudflare takes each token once.

import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { Link } from "@astryxdesign/core/Link";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Tab, TabList } from "@astryxdesign/core/TabList";
import { Text } from "@astryxdesign/core/Text";
import { TextInput } from "@astryxdesign/core/TextInput";
import {
  type Ref,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
} from "react";
import { useLegalDialog } from "@/components/legal/legal-dialog";
import { PasswordField } from "@/components/password-field";
import { useUi } from "@/components/providers";
import { useToast } from "@/components/toasts";
import {
  useVerificationT,
  verificationRefused,
} from "@/components/verification/messages";
import { VerificationField } from "@/components/verification/verification-field";
import { api, errorKey } from "@/lib/api";
import { API_ERRORS, type MessageKey } from "@/lib/messages";
import { rich } from "@/lib/rich";
import { type TurnstileState, useTurnstile } from "@/lib/turnstile";
import {
  askRegistration,
  createSubmitFlight,
  type Registration,
  tokenFor,
  waitForAnswer,
} from "./sign-in-gate";

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
  // the server's valid_email: printable ASCII only, so space, control characters and non-ASCII are refused
  if (field === "email" && /[^!-~]/.test(v)) return "account.emailAscii";
  if (field === "email" && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v))
    return "account.emailInvalid";
  if (field === "password" && v.length < 10) return "account.passwordShort";
}

/** the actions the API checks the token for (crates/api/src/account/sign_in.rs) */
const ACTION: Record<Mode, string> = { login: "sign-in", register: "register" };

type VerificationHandle = TurnstileState & { siteKey: string };

/** The widget for a site key; the dialog reads its state through `handle` when it submits. */
function Verification({
  siteKey,
  action,
  handle,
}: {
  siteKey: string;
  action: string;
  handle: Ref<VerificationHandle>;
}) {
  const verification = useTurnstile(siteKey, action);
  useImperativeHandle(handle, () => ({ ...verification, siteKey }));
  return <VerificationField verification={verification} />;
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
  const vt = useVerificationT();
  const toast = useToast();
  const legal = useLegalDialog();
  const [mode, setMode] = useState<Mode>("login");
  const [values, setValues] = useState<Partial<Record<Field, string>>>({});
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  const [canRegister, setCanRegister] = useState(true);
  const [siteKey, setSiteKey] = useState<string | null>(null);
  const refs = useRef<Partial<Record<Field, HTMLInputElement | null>>>({});
  const verification = useRef<VerificationHandle>(null);
  const value = (f: Field) => values[f] ?? "";
  // this open's registration answer, which a submit waits for; the opens so far; and the two-phase submit controller
  // (cancels a registration wait on close/mode change, keeps an issued account POST single-flight until it settles)
  const answer = useRef<Promise<Registration | null>>(Promise.resolve(null));
  const opens = useRef(0);
  const flight = useRef(createSubmitFlight());

  const apply = (a: Registration | null) => {
    // unknown: keep offering registration, without the widget
    setSiteKey(a?.site_key ?? null);
    setCanRegister(a?.open ?? true);
    if (a && !a.open) setMode("login");
  };
  // every answer of this open goes into the dialog, whether or not the submit that asked is still waiting for it; an
  // answer of an earlier open changes nothing
  const ask = () => {
    const open = opens.current;
    const p = askRegistration(() =>
      api<Registration>("/api/account/registration"),
    );
    answer.current = p;
    p.then((a) => opens.current === open && apply(a));
    return p;
  };
  // the gate asks too, for the site key only: it never offers registration
  // biome-ignore lint/correctness/useExhaustiveDependencies: asked once per open; ask only sets state
  useEffect(() => {
    opens.current++;
    if (!isOpen) {
      // unmount the widget while closed so a rotated key or recovered script failure starts clean on the next open
      setSiteKey(null);
      return;
    }
    ask();
  }, [isOpen]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: renumbered on each change of either
  useEffect(() => {
    setBusy(flight.current.onFormChange());
  }, [isOpen, mode]);
  const errorOf = (f: Field) =>
    checked ? problem(mode, f, value(f)) : undefined;

  const submit = async () => {
    if (flight.current.busy()) return;
    setChecked(true);
    const bad = FIELDS[mode].find((f) => problem(mode, f, value(f)));
    if (bad) {
      refs.current[bad]?.focus();
      return;
    }
    const attempt = flight.current.begin();
    if (!attempt) return;
    setBusy(true);
    try {
      const waited = await waitForAnswer({
        pending: answer.current,
        ask,
        still: attempt.still,
      });
      // a registration the answer says is closed is not sent either
      if (waited === "cancelled" || !attempt.still()) return;
      if (mode === "register" && waited.answer?.open === false) {
        toast({
          type: "error",
          body: t("error.registrationClosed"),
          uniqueID: "auth",
        });
        return;
      }
      const widget = verification.current;
      const token = tokenFor(waited.answer, widget);
      if (token === "required") {
        toast({ type: "error", body: vt("required"), uniqueID: "auth" });
        return;
      }
      if (!attempt.enterPost()) return;
      try {
        await api(
          mode === "login" ? "/api/account/login" : "/api/account/register",
          {
            method: "POST",
            json: {
              ...Object.fromEntries(FIELDS[mode].map((f) => [f, value(f)])),
              keep: true,
              token,
            },
          },
        );
        toast({ body: t("account.signedIn"), uniqueID: "auth" });
        setValues({});
        setChecked(false);
        onSignedIn();
      } catch (err) {
        if (!attempt.still()) return;
        toast({
          type: "error",
          body: verificationRefused(err)
            ? vt("refused")
            : t(errorKey(err, API_ERRORS, "error.failed")),
          uniqueID: "auth",
        });
      } finally {
        if (attempt.still()) widget?.reset();
      }
    } finally {
      if (attempt.finish()) setBusy(false);
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
                    <VStack gap={3}>
                      {FIELDS[mode].map((f, i) => {
                        const { label: name, type, auto } = INPUT[f];
                        const error = errorOf(f);
                        const status = error
                          ? { type: "error" as const, message: t(error) }
                          : undefined;
                        const enterKeyHint =
                          i < FIELDS[mode].length - 1 ? "next" : "done";
                        const setRef = (el: HTMLInputElement | null) => {
                          refs.current[f] = el;
                        };
                        if (f === "password") {
                          return (
                            <PasswordField
                              key={`${mode}-${f}`}
                              ref={setRef}
                              label={t(name)}
                              htmlName={f}
                              description={
                                mode === "register"
                                  ? t("account.passwordShort")
                                  : undefined
                              }
                              autoComplete={
                                mode === "login"
                                  ? "current-password"
                                  : "new-password"
                              }
                              enterKeyHint={enterKeyHint}
                              isRequired
                              isReadOnly={busy}
                              size="lg"
                              value={value(f)}
                              onChange={(v: string) =>
                                setValues((s) => ({ ...s, [f]: v }))
                              }
                              status={status}
                            />
                          );
                        }
                        const identifier =
                          f === "login" || f === "username" || f === "email";
                        return (
                          <TextInput
                            key={`${mode}-${f}`}
                            ref={setRef}
                            label={t(name)}
                            htmlName={f}
                            isRequired
                            // a submit sends the values it started with, so they cannot change while it waits or sends
                            isReadOnly={busy}
                            // the rule stays readable while typing (template form-wizard, page.tsx:453)
                            description={
                              f === "username" && mode === "register"
                                ? t("account.usernameRule")
                                : undefined
                            }
                            type={type}
                            autoComplete={auto}
                            enterKeyHint={enterKeyHint}
                            hasSpellCheck={identifier ? false : undefined}
                            hasAutoCorrect={identifier ? false : undefined}
                            autoCapitalize={identifier ? "none" : undefined}
                            value={value(f)}
                            onChange={(v: string) =>
                              setValues((s) => ({ ...s, [f]: v }))
                            }
                            size="lg"
                            status={status}
                          />
                        );
                      })}
                    </VStack>
                    {isOpen && siteKey && (
                      <Verification
                        key={`${siteKey}:${ACTION[mode]}`}
                        siteKey={siteKey}
                        action={ACTION[mode]}
                        handle={verification}
                      />
                    )}
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
                      <Text
                        type="supporting"
                        color="secondary"
                        justify="center"
                      >
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

// the check that lifts the stricter rate limits (verification/pass-host.tsx) loads through this module: its dialog,
// widget and strings are the ones these forms already load, and a lazy import of its own split a chunk of loaders
// into every page's first load (1.5 KB gzip)
export { default as PassDialog } from "@/components/verification/pass-dialog";
