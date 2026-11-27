"use client";

import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { InputGroup, type InputGroupSize } from "@astryxdesign/core/InputGroup";
import { TextInput, type TextInputStatus } from "@astryxdesign/core/TextInput";
import { type Ref, useState } from "react";
import { useUi } from "@/components/providers";
import "@/lib/icons-on-demand";

/** Password input with an inline show/hide visibility toggle inside an Astryx InputGroup. */
export function PasswordField({
  ref,
  label,
  description,
  htmlName,
  autoComplete,
  enterKeyHint,
  isRequired,
  isReadOnly,
  size = "md",
  value,
  onChange,
  status,
}: {
  ref?: Ref<HTMLInputElement>;
  label: string;
  description?: string;
  htmlName: string;
  autoComplete?: "current-password" | "new-password";
  enterKeyHint?: "next" | "done";
  isRequired?: boolean;
  isReadOnly?: boolean;
  size?: InputGroupSize;
  value: string;
  onChange: (value: string) => void;
  status?: TextInputStatus;
}) {
  const { t } = useUi();
  const [shown, setShown] = useState(false);
  return (
    <InputGroup
      label={label}
      description={description}
      isRequired={isRequired}
      size={size}
      status={status}
      style={{ width: "100%" }}
    >
      <TextInput
        ref={ref}
        label={label}
        isLabelHidden
        htmlName={htmlName}
        type={shown ? "text" : "password"}
        autoComplete={autoComplete}
        enterKeyHint={enterKeyHint}
        isRequired={isRequired}
        isReadOnly={isReadOnly}
        hasSpellCheck={false}
        hasAutoCorrect={false}
        autoCapitalize="none"
        value={value}
        onChange={onChange}
        status={status ? { type: status.type } : undefined}
      />
      <Button
        type="button"
        label={t(shown ? "account.hidePassword" : "account.showPassword")}
        isIconOnly
        variant="secondary"
        aria-pressed={shown}
        icon={<Icon icon={shown ? "eyeSlash" : "site:viewed"} size="sm" />}
        onClick={() => setShown((v) => !v)}
      />
    </InputGroup>
  );
}
