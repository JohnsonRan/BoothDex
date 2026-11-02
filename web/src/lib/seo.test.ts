import { afterEach, expect, test } from "bun:test";
import { getSiteUrl, homeMetadata } from "./seo";

const originalSiteUrl = process.env.SITE_URL;
afterEach(() => {
  if (originalSiteUrl === undefined) delete process.env.SITE_URL;
  else process.env.SITE_URL = originalSiteUrl;
});

test("site origin accepts HTTPS and local development, normalizing the trailing slash", () => {
  expect(getSiteUrl("https://catalog.example")?.href).toBe(
    "https://catalog.example/",
  );
  expect(getSiteUrl("http://localhost:3100/")?.href).toBe(
    "http://localhost:3100/",
  );
});

test("missing SITE_URL falls back locally and warns once without throwing in production", () => {
  for (const mode of ["development", "production"]) {
    const result = Bun.spawnSync(
      [
        process.execPath,
        "-e",
        'import { getSiteUrl } from "./seo"; console.log(String(getSiteUrl()?.href)); getSiteUrl();',
      ],
      {
        cwd: import.meta.dir,
        env: { ...process.env, NODE_ENV: mode, SITE_URL: "" },
      },
    );
    expect(result.exitCode).toBe(0);
    if (mode === "production") {
      expect(result.stdout.toString().trim()).toBe("undefined");
      const stderr = result.stderr.toString();
      expect(stderr).toContain("SITE_URL is not set");
      const matches = stderr.match(/SITE_URL is not set/g);
      expect(matches?.length).toBe(1);
    } else {
      expect(result.stdout.toString().trim()).toBe("http://localhost:3100/");
    }
  }
});

test("homepage metadata omits canonical and openGraph url when SITE_URL is missing in production", () => {
  const env = process.env as Record<string, string | undefined>;
  const origEnv = env.NODE_ENV;
  try {
    env.NODE_ENV = "production";
    delete process.env.SITE_URL;
    const metadata = homeMetadata(false);
    expect(metadata.metadataBase).toBeUndefined();
    expect(metadata.alternates).toBeUndefined();
    expect(metadata.openGraph?.url).toBeUndefined();
    expect(metadata.openGraph?.siteName).toBe("BoothDex");
  } finally {
    env.NODE_ENV = origEnv;
  }
});

test("malformed SITE_URL warns once and returns undefined without throwing", () => {
  for (const value of [
    "not a URL",
    "ftp://catalog.example",
    "https://user:secret@catalog.example",
    "https://catalog.example/catalog",
    "https://catalog.example/?q=dress",
    "https://catalog.example/#item",
  ]) {
    expect(getSiteUrl(value)).toBeUndefined();
  }
  const result = Bun.spawnSync(
    [
      process.execPath,
      "-e",
      'import { getSiteUrl } from "./seo"; console.log(String(getSiteUrl("bad")?.href)); getSiteUrl("also-bad");',
    ],
    {
      cwd: import.meta.dir,
      env: { ...process.env },
    },
  );
  expect(result.exitCode).toBe(0);
  expect(result.stdout.toString().trim()).toBe("undefined");
  const stderr = result.stderr.toString();
  expect(stderr).toContain("SITE_URL must be an HTTP(S) origin");
  const matches = stderr.match(/SITE_URL must be an HTTP\(S\) origin/g);
  expect(matches?.length).toBe(1);
});

test("homepage canonical and share metadata use the runtime origin", () => {
  process.env.SITE_URL = "https://first.example";
  const metadata = homeMetadata(false);
  expect(metadata.alternates?.canonical).toBe("https://first.example/");
  expect(metadata.openGraph).toMatchObject({
    url: "https://first.example/",
    siteName: "BoothDex",
    title: metadata.title,
    description: metadata.description,
  });
  expect(metadata.twitter).toMatchObject({
    card: "summary",
    title: metadata.title,
    description: metadata.description,
  });
  process.env.SITE_URL = "https://second.example/";
  expect(homeMetadata(false).alternates?.canonical).toBe(
    "https://second.example/",
  );
});

test("only the bare homepage is indexable; query views retain the root canonical", () => {
  process.env.SITE_URL = "https://catalog.example";
  expect(homeMetadata(false).robots).toEqual({ index: true, follow: true });
  expect(homeMetadata(true).robots).toEqual({ index: false, follow: true });
  expect(homeMetadata(true).alternates?.canonical).toBe(
    "https://catalog.example/",
  );
});
