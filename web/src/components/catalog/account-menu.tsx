"use client";

import {
  Bookmark,
  Heart,
  LayoutDashboard,
  Loader2,
  LogOut,
  Rss,
  Trash2,
} from "lucide-react";
import { type FormEvent, useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { api } from "@/lib/api";
import { useSettings } from "@/lib/settings";

export type Me = {
  id: number;
  username: string;
  email: string;
  display_name: string;
  is_admin: boolean;
  preferences: Record<string, string>;
};
type Saved = { id: number; name: string; query: string; rss: string };

type Props = {
  me: Me | null;
  onAuthChange: () => void;
  onShowFavourites: () => void;
  currentQuery: string;
  currentLabel: string;
};

export function AccountMenu({
  me,
  onAuthChange,
  onShowFavourites,
  currentQuery,
  currentLabel,
}: Props) {
  const { t } = useSettings();
  const [authOpen, setAuthOpen] = useState(false);
  const [savedOpen, setSavedOpen] = useState(false);

  if (!me) {
    return (
      <>
        <Button
          variant="outline"
          size="sm"
          className="shrink-0"
          onClick={() => setAuthOpen(true)}
        >
          {t("nav.signIn")}
        </Button>
        <AuthDialog
          open={authOpen}
          onOpenChange={setAuthOpen}
          onDone={() => {
            setAuthOpen(false);
            onAuthChange();
          }}
        />
      </>
    );
  }

  const logout = async () => {
    await api("/api/account/logout", { method: "POST" }).catch(() => {});
    onAuthChange();
  };

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            type="button"
            aria-label={t("nav.account")}
            className="grid size-9 shrink-0 place-items-center rounded-full bg-brand text-sm font-bold text-white"
          >
            {me.display_name.slice(0, 1).toUpperCase()}
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-56">
          <DropdownMenuLabel className="truncate">
            {me.display_name}
          </DropdownMenuLabel>
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={onShowFavourites}>
            <Heart />
            {t("account.myFavourites")}
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => setSavedOpen(true)}>
            <Rss />
            {t("account.savedSearches")}
          </DropdownMenuItem>
          {me.is_admin && (
            <DropdownMenuItem asChild>
              <a href="/admin">
                <LayoutDashboard />
                {t("account.admin")}
              </a>
            </DropdownMenuItem>
          )}
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={logout}>
            <LogOut />
            {t("account.signOut")}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <SavedSearches
        open={savedOpen}
        onOpenChange={setSavedOpen}
        currentQuery={currentQuery}
        currentLabel={currentLabel}
      />
    </>
  );
}

function AuthDialog({
  open,
  onOpenChange,
  onDone,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  onDone: () => void;
}) {
  const { t } = useSettings();
  const [busy, setBusy] = useState(false);
  const submit =
    (path: string, build: (f: FormData) => unknown) =>
    async (e: FormEvent<HTMLFormElement>) => {
      e.preventDefault();
      setBusy(true);
      try {
        await api(path, {
          method: "POST",
          json: build(new FormData(e.currentTarget)),
        });
        toast.success(t("account.signedIn"));
        onDone();
      } catch (err) {
        toast.error(err instanceof Error ? err.message : String(err));
      } finally {
        setBusy(false);
      }
    };
  const field = (f: FormData, k: string) => String(f.get(k) ?? "");
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{t("account.title")}</DialogTitle>
          <DialogDescription>{t("account.desc")}</DialogDescription>
        </DialogHeader>
        <Tabs defaultValue="login">
          <TabsList className="w-full">
            <TabsTrigger value="login">{t("account.signIn")}</TabsTrigger>
            <TabsTrigger value="register">{t("account.register")}</TabsTrigger>
          </TabsList>
          <TabsContent value="login">
            <form
              className="space-y-3 pt-2"
              onSubmit={submit("/api/account/login", (f) => ({
                login: field(f, "login"),
                password: field(f, "password"),
                keep: true,
              }))}
            >
              <Input
                name="login"
                autoComplete="username"
                placeholder={t("account.login")}
                required
                aria-label={t("account.login")}
              />
              <Input
                name="password"
                type="password"
                autoComplete="current-password"
                placeholder={t("account.password")}
                required
                aria-label={t("account.password")}
              />
              <Button type="submit" className="w-full" disabled={busy}>
                {busy && <Loader2 className="animate-spin" />}
                {t("account.signIn")}
              </Button>
            </form>
          </TabsContent>
          <TabsContent value="register">
            <form
              className="space-y-3 pt-2"
              onSubmit={submit("/api/account/register", (f) => ({
                display_name: field(f, "display_name"),
                username: field(f, "username"),
                email: field(f, "email"),
                password: field(f, "password"),
                keep: true,
              }))}
            >
              <Input
                name="display_name"
                placeholder={t("account.displayName")}
                required
                aria-label={t("account.displayName")}
              />
              <Input
                name="username"
                autoComplete="username"
                placeholder={t("account.username")}
                pattern="[A-Za-z0-9_]{3,24}"
                required
                aria-label={t("account.username")}
              />
              <Input
                name="email"
                type="email"
                autoComplete="email"
                placeholder={t("account.email")}
                required
                aria-label={t("account.email")}
              />
              <Input
                name="password"
                type="password"
                autoComplete="new-password"
                placeholder={t("account.newPassword")}
                minLength={10}
                required
                aria-label={t("account.password")}
              />
              <Button type="submit" className="w-full" disabled={busy}>
                {busy && <Loader2 className="animate-spin" />}
                {t("account.register")}
              </Button>
            </form>
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}

function SavedSearches({
  open,
  onOpenChange,
  currentQuery,
  currentLabel,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  currentQuery: string;
  currentLabel: string;
}) {
  const { t } = useSettings();
  const [items, setItems] = useState<Saved[] | null>(null);
  const [name, setName] = useState(currentLabel);
  const load = useCallback(
    () =>
      api<{ items: Saved[] }>("/api/account/searches").then((r) =>
        setItems(r.items),
      ),
    [],
  );
  useEffect(() => {
    if (open) {
      setName(currentLabel);
      load();
    }
  }, [open, currentLabel, load]);

  const save = async (e: FormEvent) => {
    e.preventDefault();
    try {
      await api("/api/account/searches", {
        method: "POST",
        json: { name, query: currentQuery || "sort=newest" },
      });
      toast.success(t("saved.saved"));
      load();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err));
    }
  };
  const remove = async (id: number) => {
    await api(`/api/account/searches/${id}`, { method: "DELETE" });
    load();
  };
  // feed links point at this site's origin, whatever the API was told
  const feedUrl = (s: Saved) => `${location.origin}/api/rss?${s.query}`;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("account.savedSearches")}</DialogTitle>
          <DialogDescription>{t("saved.desc")}</DialogDescription>
        </DialogHeader>
        <form onSubmit={save} className="flex gap-2">
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            maxLength={100}
            required
            aria-label={t("saved.name")}
          />
          <Button type="submit" className="shrink-0">
            <Bookmark />
            {t("saved.save")}
          </Button>
        </form>
        <div className="max-h-[50dvh] divide-y overflow-y-auto rounded-lg border">
          {items === null && (
            <p className="p-4 text-sm text-muted-foreground">
              {t("detail.loading")}
            </p>
          )}
          {items?.length === 0 && (
            <p className="p-4 text-sm text-muted-foreground">
              {t("saved.empty")}
            </p>
          )}
          {items?.map((s) => (
            <div key={s.id} className="flex items-center gap-2 p-3">
              <a
                href={`/?${s.query}`}
                className="min-w-0 flex-1 truncate text-sm hover:underline"
              >
                {s.name}
              </a>
              <Button
                size="sm"
                variant="outline"
                onClick={() =>
                  navigator.clipboard
                    .writeText(feedUrl(s))
                    .then(() => toast.success(t("saved.copied")))
                }
              >
                <Rss />
                {t("saved.copy")}
              </Button>
              <Button
                size="icon-sm"
                variant="ghost"
                aria-label={t("saved.delete", { name: s.name })}
                onClick={() => remove(s.id)}
              >
                <Trash2 />
              </Button>
            </div>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
