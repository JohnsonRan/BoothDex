// Shapes of the admin API (crates/api/src/admin.rs, worker jobs.rs) and the fixed choices the console offers.

export const KINDS = [
  "incremental",
  "details",
  "refresh",
  "sweep",
  "enrich",
  "embed",
  "localize",
] as const;
export type Kind = (typeof KINDS)[number];

export const STATUSES = [
  "queued",
  "running",
  "cancelling",
  "cancelled",
  "succeeded",
  "failed",
] as const;
export type JobStatus = (typeof STATUSES)[number];

export type Job = {
  id: number;
  kind: Kind;
  status: JobStatus;
  params: Record<string, unknown>;
  done: number;
  total: number | null;
  message: string | null;
  error: string | null;
  created_by: string;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  log?: string;
};

export type Schedule = {
  kind: Kind;
  enabled: boolean;
  interval_minutes: number;
  params: Record<string, unknown>;
  next_run_at: string;
};

export type Overview = {
  now: string;
  data: {
    items: number;
    listed: number;
    delisted: number;
    needs_enrichment: number;
    published_24h: number;
    refreshed_24h: number;
    stalest_scrape: string | null;
    missing_images: number;
    users: number;
    favourites: number;
    saved_searches: number;
    db_bytes: number;
  };
  catalog: {
    version: number;
    db_version: number;
    docs: number;
    bitmap_bytes: number;
    loaded_at: string;
    build_secs: number;
  };
  worker: {
    alive: boolean;
    heartbeat_at: string | null;
    started_at: string | null;
    current_job: number | null;
    booth_requests: number;
    booth_errors: number;
    booth_rate_limited: number;
    rss_bytes: number | null;
  };
  live_jobs: Job[];
  system: {
    cpu_percent: number;
    cpu_count: number;
    mem_total: number;
    mem_used: number;
    load_avg: number | null;
    api_rss: number | null;
    api_cpu_percent: number | null;
    disk_total: number | null;
    disk_free: number | null;
    disk_mount: string | null;
  } | null;
  imgproxy: {
    reachable: boolean;
    requests?: number;
    hit_rate?: number;
    errors?: number;
    bytes_out?: number;
    disk_cache_mb?: number;
    memory_cache_mb?: number;
  } | null;
};

export type Ai = {
  base_url?: string | null;
  model?: string | null;
  api_key_set: boolean;
  api_key_hint?: string | null;
  reasoning_effort?: string;
  embed_model?: string | null;
  from_env?: Partial<
    Record<
      "base_url" | "model" | "api_key" | "reasoning_effort" | "embed_model",
      boolean
    >
  >;
};

/** The 8 schedule presets, in minutes. */
export const INTERVALS = [15, 30, 60, 180, 360, 720, 1440, 10080];

export const EFFORTS = [
  "none",
  "minimal",
  "low",
  "medium",
  "high",
  "xhigh",
] as const;

/** The safe run options per kind (owner ruling): flags and counts with the worker's defaults. */
type RunParam =
  | { name: "full" | "force"; type: "flag" }
  | { name: "max_pages" | "batch"; type: "count"; fallback?: number };
export const RUN_PARAMS: Partial<Record<Kind, RunParam[]>> = {
  incremental: [
    { name: "max_pages", type: "count", fallback: 40 },
    { name: "full", type: "flag" },
  ],
  refresh: [{ name: "batch", type: "count", fallback: 1500 }],
  sweep: [{ name: "max_pages", type: "count" }],
  enrich: [{ name: "batch", type: "count", fallback: 200 }],
  embed: [{ name: "batch", type: "count", fallback: 64 }],
  localize: [{ name: "force", type: "flag" }],
};
