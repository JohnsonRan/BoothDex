"use client";
// Users: GET /api/admin/users, newest first, 50 a page. Laid out as the crawler history (history.tsx): the
// search field of template table-filter (page.tsx:3419-3430, size sm with the search glyph) above the template
// incident-console's rows (page.tsx:347-380: List with dividers, Tokens and a relative Timestamp at the end, the
// row opens the details) and its EmptyState (:325-332); Pagination under the rows. The search and page are in
// the URL (?q=&page=), so Back restores the view.

import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Icon } from "@astryxdesign/core/Icon";
import { HStack, StackItem } from "@astryxdesign/core/Layout";
import { List, ListItem } from "@astryxdesign/core/List";
import { Pagination } from "@astryxdesign/core/Pagination";
import { Spinner } from "@astryxdesign/core/Spinner";
import { Text } from "@astryxdesign/core/Text";
import { TextInput } from "@astryxdesign/core/TextInput";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import { errorText, useAdminT } from "./messages";
import { PageFrame } from "./page-frame";
import type { AdminUser } from "./types";
import { UserDialog, UserTokens } from "./user-dialog";

const PAGE = 50;
const PAUSE_MS = 300;

function UserList() {
  const t = useAdminT();
  const router = useRouter();
  const path = usePathname();
  const query = useSearchParams();
  const q = query.get("q") ?? "";
  const page = Math.max(1, Number(query.get("page")) || 1);
  const [text, setText] = useState(q);
  const [data, setData] = useState<{ total: number; users: AdminUser[] }>();
  const [error, setError] = useState<unknown>();
  const [openId, setOpenId] = useState<number>();
  // bumped after a change in the dialog, so the page is read again
  const [version, setVersion] = useState(0);

  const go = (next: { q?: string; page?: number }) => {
    const params = new URLSearchParams(query);
    for (const [k, v] of Object.entries(next)) {
      if (!v || v === 1) params.delete(k);
      else params.set(k, String(v));
    }
    const qs = params.toString();
    router.replace(qs ? `${path}?${qs}` : path, { scroll: false });
  };

  // a pause in typing searches, from the first page
  // biome-ignore lint/correctness/useExhaustiveDependencies: only the typed text starts a search
  useEffect(() => {
    const term = text.trim();
    if (term === q) return;
    const timer = setTimeout(() => go({ q: term, page: 1 }), PAUSE_MS);
    return () => clearTimeout(timer);
  }, [text]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: version only asks for the same page again
  useEffect(() => {
    let stale = false;
    const params = new URLSearchParams({
      q,
      offset: String((page - 1) * PAGE),
    });
    api<{ total: number; users: AdminUser[] }>(
      `/api/admin/users?${params}`,
    ).then(
      (d) => {
        if (stale) return;
        setData(d);
        setError(undefined);
      },
      (e) => {
        if (!stale) setError(e);
      },
    );
    return () => {
      stale = true;
    };
  }, [q, page, version]);

  return (
    <>
      <HStack gap={2} vAlign="center" wrap="wrap">
        <StackItem size="fill">
          <TextInput
            label={t("users.search")}
            isLabelHidden
            placeholder={t("users.searchHint")}
            size="sm"
            width="100%"
            startIcon="search"
            hasClear
            value={text}
            onChange={setText}
          />
        </StackItem>
        {data && (
          <Text type="supporting" color="secondary">
            {t("users.count", { n: data.total })}
          </Text>
        )}
      </HStack>
      {error ? (
        <EmptyState
          title={errorText(t, error)}
          icon={<Icon icon="admin:failed" size="lg" />}
        />
      ) : !data ? (
        <Spinner label={t("nav.users")} />
      ) : data.users.length === 0 ? (
        <EmptyState
          title={q ? t("users.noMatch") : t("users.empty")}
          icon={<Icon icon="admin:empty" size="lg" />}
        />
      ) : (
        <>
          <List density="compact" hasDividers edge="outset">
            {data.users.map((u) => (
              <ListItem
                key={u.id}
                label={`${u.display_name} @${u.username}`}
                description={u.email}
                endContent={
                  <HStack gap={3} vAlign="center">
                    <UserTokens user={u} />
                    {u.last_sign_in_at && (
                      <Timestamp
                        value={u.last_sign_in_at}
                        format="relative"
                        color="secondary"
                      />
                    )}
                  </HStack>
                }
                onClick={() => setOpenId(u.id)}
              />
            ))}
          </List>
          <Pagination
            page={page}
            onChange={(p) => go({ page: p })}
            totalItems={data.total}
            pageSize={PAGE}
            size="sm"
          />
        </>
      )}
      {openId !== undefined && (
        <UserDialog
          id={openId}
          onClose={() => setOpenId(undefined)}
          onChange={() => setVersion((v) => v + 1)}
        />
      )}
    </>
  );
}

export function UsersPage() {
  const t = useAdminT();
  return (
    <PageFrame title={t("nav.users")}>
      <UserList />
    </PageFrame>
  );
}
