"use client";
// Beside the overview title: the stream's connection, the worker's state and the server time of the last
// update. Reconnecting is a warning (EventSource retries by itself), an offline worker an error. A StatusDot with its text, as the Astryx template incident-console pairs them
// (templates/pages/incident-console/page.tsx:398-411).

import { HStack } from "@astryxdesign/core/Layout";
import { StatusDot } from "@astryxdesign/core/StatusDot";
import { Text } from "@astryxdesign/core/Text";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { useLive } from "./admin-console";
import { useAdminT } from "./messages";

function Dot({
  ok,
  on,
  off,
  failure,
}: {
  ok: boolean;
  on: string;
  off: string;
  failure: "warning" | "error";
}) {
  const label = ok ? on : off;
  return (
    <HStack gap={1} vAlign="center">
      <StatusDot variant={ok ? "success" : failure} label={label} />
      <Text type="supporting" color="secondary">
        {label}
      </Text>
    </HStack>
  );
}

export function LiveStatus() {
  const t = useAdminT();
  const { overview, connected } = useLive();
  return (
    <HStack gap={3} vAlign="center" wrap="wrap">
      <Dot
        ok={connected}
        on={t("live.on")}
        off={t("live.off")}
        failure="warning"
      />
      <Dot
        ok={overview.worker.alive}
        on={t("worker.on")}
        off={t("worker.off")}
        failure="error"
      />
      <Text type="supporting" color="secondary">
        {t("updated")}{" "}
        <Timestamp
          value={overview.now}
          format="time"
          type="supporting"
          color="secondary"
        />
      </Text>
    </HStack>
  );
}
