"use client";
// How a text search was answered (words, meaning, a cached query embedding) and where its time went. The readout's
// count opens it; the popover follows block PopoverKeyboardShortcuts (a heading, a divider, then a row of text
// columns per entry: PopoverKeyboardShortcuts.tsx:18-37).

import { Divider } from "@astryxdesign/core/Divider";
import { Link } from "@astryxdesign/core/Link";
import { Popover } from "@astryxdesign/core/Popover";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Heading, Text } from "@astryxdesign/core/Text";
import { useUi } from "@/components/providers";
import type { SemanticStatus, TraceStep } from "@/lib/api-types";
import type { MessageKey } from "@/lib/messages";

const STEPS = new Set([
  "favourites",
  "settings",
  "embed_memory",
  "embed_db",
  "embed_provider",
  "embed_timeout",
  "embed_failed",
  "neighbours",
  "search",
  "items",
]);
const FAILED = new Set(["embed_timeout", "embed_failed"]);

const ms = (n: number) => `${Math.round(n)} ms`;

export function SearchTrace({
  count,
  semantic,
  steps,
  serverMs,
  totalMs,
}: {
  /** the readout's text, which becomes the trigger */
  count: string;
  semantic: SemanticStatus;
  steps: TraceStep[];
  /** the API's own time */
  serverMs: number;
  /** the page server's request, API included */
  totalMs: number;
}) {
  const { t } = useUi();
  const cached = steps.some(
    (s) => s.step === "embed_memory" || s.step === "embed_db",
  );
  const mode: MessageKey =
    semantic === "used"
      ? cached
        ? "trace.mode.cached"
        : "trace.mode.semantic"
      : semantic === "unavailable"
        ? "trace.mode.unavailable"
        : "trace.mode.words";
  let sum = 0;
  return (
    <Popover
      placement="below"
      label={t("trace.title")}
      width={320}
      content={
        <VStack gap={2}>
          <VStack gap={0}>
            <Heading level={4}>{t("trace.title")}</Heading>
            <Text type="supporting" color="secondary">
              {t(mode)}
            </Text>
          </VStack>
          <Divider />
          {steps.map((s, i) => {
            sum += s.ms;
            return (
              <HStack
                // steps can repeat in principle; the position keeps keys unique
                // biome-ignore lint/suspicious/noArrayIndexKey: steps have no id
                key={i}
                gap={3}
                style={
                  FAILED.has(s.step)
                    ? { color: "var(--color-text-error)" }
                    : undefined
                }
              >
                <Text type="body" color="inherit" style={{ flex: 1 }}>
                  {STEPS.has(s.step)
                    ? t(`trace.step.${s.step}` as MessageKey)
                    : s.step}
                </Text>
                <Text type="body" color="inherit" hasTabularNumbers>
                  +{ms(s.ms)}
                </Text>
                <Text type="body" color="secondary" hasTabularNumbers>
                  {ms(sum)}
                </Text>
              </HStack>
            );
          })}
          <Divider />
          <HStack gap={4} style={{ justifyContent: "space-between" }}>
            <Text type="supporting" color="secondary" hasTabularNumbers>
              {t("trace.server")} {ms(serverMs)}
            </Text>
            <Text type="supporting" color="secondary" hasTabularNumbers>
              {t("trace.total")} {ms(totalMs)}
            </Text>
          </HStack>
        </VStack>
      }
    >
      {/* the latency stays on the trigger, so the time shows without opening the trace */}
      <Link type="supporting" color="secondary">
        {count} · {ms(totalMs)}
      </Link>
    </Popover>
  );
}
