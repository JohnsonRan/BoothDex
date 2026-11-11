// The privacy policy and terms of use (/privacy, /terms): one text per interface language (legal-<lang>.ts), read
// on the server only. The texts describe what the software does; the operator of an instance names themselves
// and their contact address through OPERATOR_NAME and CONTACT_EMAIL (docs/DEPLOY.md), read at runtime.
// In the texts, {operator}, {contact}, {source} and {privacy} stand for the operator's name, their address (a
// mailto link), the source repository and a link to the privacy policy.
// Same style rules as the UI strings (messages.ts).

import type { UiLang } from "./messages";
import { getSiteUrl } from "./seo";

/** A paragraph, a bulleted list, or a table (the cookie list). */
export type LegalBlock =
  | string
  | { list: string[] }
  | { table: { columns: string[]; rows: string[][] } };

type LegalDoc = {
  title: string;
  /** the lead paragraph under the title */
  summary: string;
  sections: { id: string; heading: string; body: LegalBlock[] }[];
};

export type LegalTexts = {
  privacy: LegalDoc;
  terms: LegalDoc;
  /** shown for {contact} while CONTACT_EMAIL is not set */
  noContact: string;
};

export type LegalKind = "privacy" | "terms";

/** One text as the browser gets it (legal-body.tsx): plain data, so a server action can return it. */
export type LegalView = LegalDoc & {
  /** the "Last updated" line */
  updated: string;
  /** what {operator}, {contact}, {source} and {privacy} stand for: the address without its link, the link's label */
  fill: {
    operator: string;
    contact?: string;
    noContact: string;
    source: string;
    privacy: string;
  };
};

/** The day the texts last changed in substance; update it with them. */
export const LEGAL_UPDATED = "2026-11-11";

export const SOURCE_URL = "https://github.com/JohnsonRan/BoothDex";

const TEXTS: Record<UiLang, () => Promise<{ default: LegalTexts }>> = {
  zh: () => import("./legal-zh"),
  en: () => import("./legal-en"),
  ja: () => import("./legal-ja"),
  ko: () => import("./legal-ko"),
};

export const legalTexts = async (ui: UiLang) => (await TEXTS[ui]()).default;

let warnedNoContact = false;

/** The operator's name: OPERATOR_NAME, else the site's host name. */
export function operatorName(): string {
  return process.env.OPERATOR_NAME?.trim() || getSiteUrl()?.host || "BoothDex";
}

/** The operator's address (CONTACT_EMAIL), if set and plausible. */
export function contactEmail(): string | undefined {
  const value = process.env.CONTACT_EMAIL?.trim();
  if (value && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value)) return value;
  if (process.env.NODE_ENV === "production" && !warnedNoContact) {
    warnedNoContact = true;
    console.warn(
      "CONTACT_EMAIL is not set or invalid; /privacy and /terms show no contact address",
    );
  }
  return undefined;
}
