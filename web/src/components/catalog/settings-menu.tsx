"use client";

import { Coins, Languages, Moon, Settings2, Sun, Type } from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { MESSAGES, UI_LANGS, type UiLang } from "@/lib/messages";
import {
  type ContentLang,
  CURRENCIES,
  type Currency,
  type Theme,
  useSettings,
} from "@/lib/settings";

const CONTENT_LANGS: ContentLang[] = ["original", "zh", "en", "ja", "ko"];

/** Interface language, item-text language, currency and theme. Saved per browser and, when signed in, per account. */
export function SettingsMenu() {
  const s = useSettings();
  const { t } = s;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={t("settings.title")}
          className="grid size-9 shrink-0 place-items-center rounded-lg border bg-card hover:bg-accent"
        >
          <Settings2 className="size-4" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-64">
        <DropdownMenuLabel>{t("settings.title")}</DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuSub>
          <DropdownMenuSubTrigger>
            <Languages />
            <span className="flex-1">{t("settings.ui")}</span>
            <span className="text-xs text-muted-foreground">
              {MESSAGES[s.ui]["lang.name"]}
            </span>
          </DropdownMenuSubTrigger>
          <DropdownMenuSubContent>
            <DropdownMenuRadioGroup
              value={s.ui}
              onValueChange={(v) => s.set({ ui: v as UiLang })}
            >
              {UI_LANGS.map((l) => (
                <DropdownMenuRadioItem key={l} value={l}>
                  {MESSAGES[l]["lang.name"]}
                </DropdownMenuRadioItem>
              ))}
            </DropdownMenuRadioGroup>
          </DropdownMenuSubContent>
        </DropdownMenuSub>
        <DropdownMenuSub>
          <DropdownMenuSubTrigger>
            <Type />
            <span className="flex-1">{t("settings.content")}</span>
            <span className="text-xs text-muted-foreground">
              {s.content === "original"
                ? t("settings.contentOriginal")
                : MESSAGES[s.content]["lang.name"]}
            </span>
          </DropdownMenuSubTrigger>
          <DropdownMenuSubContent>
            <DropdownMenuRadioGroup
              value={s.content}
              onValueChange={(v) => s.set({ content: v as ContentLang })}
            >
              {CONTENT_LANGS.map((l) => (
                <DropdownMenuRadioItem key={l} value={l}>
                  {l === "original"
                    ? t("settings.contentOriginal")
                    : MESSAGES[l]["lang.name"]}
                </DropdownMenuRadioItem>
              ))}
            </DropdownMenuRadioGroup>
          </DropdownMenuSubContent>
        </DropdownMenuSub>
        <DropdownMenuSub>
          <DropdownMenuSubTrigger>
            <Coins />
            <span className="flex-1">{t("settings.currency")}</span>
            <span className="text-xs text-muted-foreground">{s.currency}</span>
          </DropdownMenuSubTrigger>
          <DropdownMenuSubContent className="max-h-80 overflow-y-auto">
            <DropdownMenuRadioGroup
              value={s.currency}
              onValueChange={(v) => s.set({ currency: v as Currency })}
            >
              {CURRENCIES.map((c) => (
                <DropdownMenuRadioItem key={c} value={c}>
                  {c}
                </DropdownMenuRadioItem>
              ))}
            </DropdownMenuRadioGroup>
          </DropdownMenuSubContent>
        </DropdownMenuSub>
        <DropdownMenuSeparator />
        <DropdownMenuLabel className="text-xs font-normal text-muted-foreground">
          {t("settings.theme")}
        </DropdownMenuLabel>
        <DropdownMenuRadioGroup
          value={s.theme}
          onValueChange={(v) => s.set({ theme: v as Theme })}
        >
          <DropdownMenuRadioItem value="dark">
            <Moon />
            {t("settings.dark")}
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="light">
            <Sun />
            {t("settings.light")}
          </DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
        <DropdownMenuSeparator />
        <div className="space-y-1 p-1 text-[11px] text-muted-foreground">
          <div className="px-1.5 pt-0.5">
            © 2026 BoothDex contributors · AGPL-3.0-or-later
          </div>
          <div className="flex items-center justify-between gap-1">
            <DropdownMenuItem
              asChild
              className="h-6 px-1.5 text-[11px] text-muted-foreground"
            >
              <a
                href="https://github.com/JohnsonRan/BoothDex"
                target="_blank"
                rel="noopener"
              >
                {t("settings.source")}
              </a>
            </DropdownMenuItem>
            <DropdownMenuItem
              asChild
              className="h-6 px-1.5 text-[11px] text-muted-foreground"
            >
              <a
                href="https://hyperos.mi.com/font/"
                target="_blank"
                rel="noopener"
              >
                {t("settings.fontCredit")}
              </a>
            </DropdownMenuItem>
          </div>
        </div>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
