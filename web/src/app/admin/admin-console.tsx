"use client";

import {
  Activity,
  Cpu,
  Database,
  HardDrive,
  ImageIcon,
  Loader2,
  Play,
  RefreshCw,
  ScrollText,
  Square,
  Wifi,
  WifiOff,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { ApiError, api, fmt } from "@/lib/api";
import { AiSettingsCard } from "./ai-settings-card";
import { Centered } from "./centered";
import { Login } from "./login";
import { ResetPasswordCard } from "./reset-password-card";

type Job = {
  id: number;
  kind: Kind;
  status:
    | "queued"
    | "running"
    | "cancelling"
    | "cancelled"
    | "succeeded"
    | "failed";
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
type Kind =
  | "incremental"
  | "details"
  | "refresh"
  | "sweep"
  | "enrich"
  | "localize"
  | "embed";
type Schedule = {
  kind: Kind;
  enabled: boolean;
  interval_minutes: number;
  params: Record<string, unknown>;
  next_run_at: string;
};
type Overview = {
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

const KIND: Record<Kind, { label: string; desc: string }> = {
  incremental: {
    label: "增量抓取",
    desc: "翻 Booth 最新列表，收录新商品和图片链接，追上后自动停止",
  },
  details: {
    label: "补抓缺图",
    desc: "为没有 Booth 图片链接的商品逐个抓取商品页",
  },
  refresh: {
    label: "刷新热门商品",
    desc: "按“热度 × 多久没更新”排序，逐个重新抓取商品页，更新价格、点赞、变体和下架状态",
  },
  sweep: {
    label: "全量巡检",
    desc: "把 Booth 的 VRChat 列表从头翻到尾(约 3,400 页，1 小时)，更新所有商品的价格，并找出已下架的商品",
  },
  enrich: {
    label: "AI 处理",
    desc: "为新商品生成分类、分级、推断价格和中英日韩翻译，并按名称匹配适配模型(每批 200 个，有积压时自动继续下一批，取消任务即可停止；在下方“AI 处理设置”配置 AI 模型之前不会运行)",
  },
  embed: {
    label: "语义向量",
    desc: "为商品生成用于语义搜索的向量(新商品和改过标题或简介的商品；在下方设置向量模型之前不会运行)",
  },
  localize: {
    label: "翻译分类",
    desc: "把分类和标签名翻译成中文(只翻译还没有中文名的)。保存 AI 设置后会自动运行一次",
  },
};
const STATUS: Record<Job["status"], { label: string; cls: string }> = {
  queued: { label: "排队中", cls: "bg-muted text-muted-foreground" },
  running: { label: "运行中", cls: "bg-primary/15 text-primary" },
  cancelling: {
    label: "停止中",
    cls: "bg-[var(--warning)]/15 text-[var(--warning)]",
  },
  cancelled: { label: "已取消", cls: "bg-muted text-muted-foreground" },
  succeeded: {
    label: "成功",
    cls: "bg-[var(--success)]/15 text-[var(--success)]",
  },
  failed: { label: "失败", cls: "bg-destructive/15 text-destructive" },
};
const INTERVALS = [15, 30, 60, 180, 360, 720, 1440, 10080];
const intervalLabel = (m: number) =>
  m < 60
    ? `${m} 分钟`
    : m < 1440
      ? `${m / 60} 小时`
      : m === 10080
        ? "每周"
        : `${m / 1440} 天`;

export function AdminConsole() {
  const [auth, setAuth] = useState<"checking" | "login" | "forbidden" | "ok">(
    "checking",
  );
  const [ov, setOv] = useState<Overview | null>(null);
  const [connected, setConnected] = useState(false);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [schedules, setSchedules] = useState<Schedule[]>([]);
  const [logJob, setLogJob] = useState<number | null>(null);

  const check = useCallback(async () => {
    try {
      setOv(await api<Overview>("/api/admin/overview"));
      setAuth("ok");
    } catch (e) {
      setAuth(
        e instanceof ApiError && e.status === 403 ? "forbidden" : "login",
      );
    }
  }, []);
  useEffect(() => {
    check();
  }, [check]);

  // live overview over SSE; EventSource reconnects by itself
  useEffect(() => {
    if (auth !== "ok") return;
    const es = new EventSource("/api/admin/stream");
    es.addEventListener("overview", (e) => {
      setOv(JSON.parse((e as MessageEvent).data));
      setConnected(true);
    });
    es.onerror = () => setConnected(false);
    return () => es.close();
  }, [auth]);

  const loadJobs = useCallback(async () => {
    const j = await api<{ jobs: Job[]; schedules: Schedule[] }>(
      "/api/admin/jobs",
    );
    setJobs(j.jobs);
    setSchedules(j.schedules);
  }, []);
  useEffect(() => {
    if (auth !== "ok") return;
    loadJobs();
    const t = setInterval(loadJobs, 5000);
    return () => clearInterval(t);
  }, [auth, loadJobs]);

  const act = async (fn: () => Promise<unknown>, ok: string) => {
    try {
      await fn();
      toast.success(ok);
      loadJobs();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  };
  const start = (kind: Kind) =>
    act(
      () =>
        api("/api/admin/jobs", { method: "POST", json: { kind, params: {} } }),
      `已加入队列：${KIND[kind].label}`,
    );
  const cancel = (id: number) =>
    act(
      () => api(`/api/admin/jobs/${id}/cancel`, { method: "POST" }),
      "已请求停止",
    );
  const schedule = (
    kind: Kind,
    patch: Partial<Schedule> & { run_now?: boolean },
    ok: string,
  ) =>
    act(
      () => api(`/api/admin/schedules/${kind}`, { method: "PUT", json: patch }),
      ok,
    );
  const reload = () =>
    act(
      () => api("/api/admin/reload", { method: "POST" }),
      "已请求重建索引，约 20 秒后生效",
    );

  if (auth === "checking")
    return (
      <Centered>
        <Loader2 className="size-6 animate-spin text-muted-foreground" />
      </Centered>
    );
  if (auth === "login") return <Login onDone={check} />;
  if (auth === "forbidden")
    return (
      <Centered>
        <p className="text-muted-foreground">当前账号不是管理员。</p>
      </Centered>
    );
  if (!ov) return null;

  const d = ov.data;
  const sys = ov.system;
  const live = ov.live_jobs;

  return (
    <div className="mx-auto w-full max-w-7xl space-y-6 p-4 pb-16 md:p-8">
      <header className="flex flex-wrap items-center gap-3">
        <h1 className="text-xl font-semibold tracking-tight md:text-2xl">
          管理后台
        </h1>
        <Badge variant="outline" className="gap-1.5">
          {connected ? (
            <Wifi className="size-3 text-[var(--success)]" />
          ) : (
            <WifiOff className="size-3 text-destructive" />
          )}
          {connected ? "实时" : "重连中"}
        </Badge>
        <Badge variant="outline" className="gap-1.5">
          <span
            className={`size-2 rounded-full ${ov.worker.alive ? "bg-[var(--success)]" : "bg-destructive"}`}
          />
          Worker {ov.worker.alive ? "运行中" : "离线"}
        </Badge>
        <div className="ml-auto text-xs text-muted-foreground">
          更新于 {new Date(ov.now).toLocaleTimeString("zh-CN")}
        </div>
      </header>

      <section className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <Stat
          label="商品总数"
          value={fmt.num(d.items)}
          sub={`在售 ${fmt.num(d.listed)} · 下架 ${fmt.num(d.delisted)}`}
        />
        <Stat
          label="24 小时新上架"
          value={fmt.num(d.published_24h)}
          sub={`24 小时内刷新 ${fmt.num(d.refreshed_24h)}`}
        />
        <Stat
          label="待 AI 处理"
          value={fmt.num(d.needs_enrichment)}
          sub="不会出现在模型适配和分类筛选里"
          warn={d.needs_enrichment > 0}
        />
        <Stat
          label="缺 Booth 图片"
          value={fmt.num(d.missing_images)}
          sub={`最久未刷新 ${fmt.ago(d.stalest_scrape, Date.parse(ov.now))}`}
          warn={d.missing_images > 0}
        />
        <Stat
          label="用户"
          value={fmt.num(d.users)}
          sub={`收藏 ${fmt.num(d.favourites)} · 订阅 ${fmt.num(d.saved_searches)}`}
        />
        <Stat
          label="搜索索引"
          value={`v${ov.catalog.version}`}
          sub={
            ov.catalog.version === ov.catalog.db_version
              ? `${fmt.num(ov.catalog.docs)} 件 · ${fmt.ago(ov.catalog.loaded_at, Date.parse(ov.now))}构建`
              : "有新数据，等待重建…"
          }
          action={
            <Button size="sm" variant="outline" onClick={reload}>
              <RefreshCw className="size-3.5" />
              重建
            </Button>
          }
        />
        <Stat
          label="数据库"
          value={fmt.bytes(d.db_bytes)}
          sub={`索引位图 ${fmt.bytes(ov.catalog.bitmap_bytes)}`}
        />
        <Stat
          label="Booth 请求"
          value={fmt.num(ov.worker.booth_requests)}
          sub={`错误 ${fmt.num(ov.worker.booth_errors)} · 限流 ${fmt.num(ov.worker.booth_rate_limited)}`}
          warn={ov.worker.booth_rate_limited > 0}
        />
      </section>

      <section className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        <Meter
          icon={<Cpu className="size-4" />}
          label="CPU"
          value={sys?.cpu_percent ?? 0}
          detail={
            sys
              ? `${sys.cpu_count} 核 · 负载 ${sys.load_avg?.toFixed(2) ?? "—"} · API ${sys.api_cpu_percent?.toFixed(0) ?? "—"}%`
              : "—"
          }
        />
        <Meter
          icon={<Activity className="size-4" />}
          label="内存"
          value={sys ? (sys.mem_used / sys.mem_total) * 100 : 0}
          detail={
            sys
              ? `${fmt.bytes(sys.mem_used)} / ${fmt.bytes(sys.mem_total)} · API ${fmt.bytes(sys.api_rss)} · Worker ${fmt.bytes(ov.worker.rss_bytes)}`
              : "—"
          }
        />
        <Meter
          icon={<HardDrive className="size-4" />}
          label="磁盘"
          value={
            sys?.disk_total
              ? ((sys.disk_total - (sys.disk_free ?? 0)) / sys.disk_total) * 100
              : 0
          }
          detail={
            sys?.disk_total
              ? `剩余 ${fmt.bytes(sys.disk_free)} / ${fmt.bytes(sys.disk_total)} · ${sys.disk_mount}`
              : "—"
          }
        />
        <Meter
          icon={<ImageIcon className="size-4" />}
          label="图片缓存命中率"
          value={(ov.imgproxy?.hit_rate ?? 0) * 100}
          detail={
            ov.imgproxy?.reachable
              ? `${fmt.num(ov.imgproxy.requests)} 次请求 · 输出 ${fmt.bytes(ov.imgproxy.bytes_out)} · 错误 ${fmt.num(ov.imgproxy.errors)}`
              : "图片服务不可达"
          }
          bad={!ov.imgproxy?.reachable}
        />
      </section>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Database className="size-4" />
            抓取任务
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          {(Object.keys(KIND) as Kind[]).map((kind) => {
            const job = live.find((j) => j.kind === kind);
            const sc = schedules.find((s) => s.kind === kind);
            const pct = job?.total ? (job.done / job.total) * 100 : 0;
            return (
              <div key={kind} className="rounded-lg border p-3 md:p-4">
                <div className="flex flex-wrap items-start gap-3">
                  <div className="min-w-0 flex-1 basis-60">
                    <div className="flex items-center gap-2 font-medium">
                      {KIND[kind].label}
                      {job && <StatusBadge s={job.status} />}
                    </div>
                    <p className="mt-0.5 text-xs text-muted-foreground">
                      {KIND[kind].desc}
                    </p>
                  </div>
                  {sc && (
                    <div className="flex items-center gap-2 text-sm">
                      <Switch
                        checked={sc.enabled}
                        onCheckedChange={(v) =>
                          schedule(
                            kind,
                            { enabled: v },
                            v ? "已开启定时" : "已关闭定时",
                          )
                        }
                        aria-label={`${KIND[kind].label}定时运行`}
                      />
                      <Select
                        value={String(sc.interval_minutes)}
                        onValueChange={(v) =>
                          schedule(
                            kind,
                            { interval_minutes: Number(v) },
                            "已更新间隔",
                          )
                        }
                      >
                        <SelectTrigger
                          size="sm"
                          className="w-28"
                          aria-label="运行间隔"
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {INTERVALS.map((m) => (
                            <SelectItem key={m} value={String(m)}>
                              每 {intervalLabel(m)}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                  )}
                  <div className="flex gap-2">
                    {job ? (
                      <>
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => setLogJob(job.id)}
                        >
                          <ScrollText className="size-3.5" />
                          日志
                        </Button>
                        <Button
                          size="sm"
                          variant="destructive"
                          disabled={job.status === "cancelling"}
                          onClick={() => cancel(job.id)}
                        >
                          <Square className="size-3.5" />
                          停止
                        </Button>
                      </>
                    ) : (
                      <Button size="sm" onClick={() => start(kind)}>
                        <Play className="size-3.5" />
                        立即运行
                      </Button>
                    )}
                  </div>
                </div>
                {job?.status === "running" || job?.status === "cancelling" ? (
                  <div className="mt-3 space-y-1.5">
                    <Progress value={pct} className="h-1.5" />
                    <div className="flex flex-wrap justify-between gap-2 text-xs text-muted-foreground">
                      <span>{job.message ?? "启动中…"}</span>
                      <span className="tabular-nums">
                        {fmt.num(job.done)}
                        {job.total ? ` / ${fmt.num(job.total)}` : ""} · 已运行{" "}
                        {fmt.duration(job.started_at)}
                        {job.total && job.done > 0 && job.started_at
                          ? ` · 预计还要 ${eta(job)}`
                          : ""}
                      </span>
                    </div>
                  </div>
                ) : sc?.enabled ? (
                  <p className="mt-2 text-xs text-muted-foreground">
                    下次运行：{new Date(sc.next_run_at).toLocaleString("zh-CN")}
                  </p>
                ) : null}
              </div>
            );
          })}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>最近任务</CardTitle>
        </CardHeader>
        <CardContent className="divide-y p-0">
          {jobs.length === 0 && (
            <p className="p-6 text-sm text-muted-foreground">还没有任务。</p>
          )}
          {jobs.slice(0, 30).map((j) => (
            <button
              key={j.id}
              type="button"
              onClick={() => setLogJob(j.id)}
              className="flex w-full flex-wrap items-center gap-x-3 gap-y-1 px-4 py-3 text-left text-sm hover:bg-muted/40 md:px-6"
            >
              <span className="w-12 text-muted-foreground tabular-nums">
                #{j.id}
              </span>
              <span className="w-20 font-medium">
                {KIND[j.kind]?.label ?? j.kind}
              </span>
              <StatusBadge s={j.status} />
              <span className="min-w-0 flex-1 basis-48 truncate text-muted-foreground">
                {j.error ?? j.message ?? ""}
              </span>
              <span className="text-xs text-muted-foreground tabular-nums">
                {fmt.ago(j.created_at, Date.parse(ov.now))} ·{" "}
                {j.started_at
                  ? fmt.duration(j.started_at, j.finished_at)
                  : "未开始"}{" "}
                · {j.created_by}
              </span>
            </button>
          ))}
        </CardContent>
      </Card>

      <AiSettingsCard />

      <ResetPasswordCard />

      <JobLog id={logJob} onClose={() => setLogJob(null)} />
    </div>
  );
}

function eta(j: Job) {
  if (!j.total || !j.started_at || j.done === 0) return "—";
  const elapsed = (Date.now() - Date.parse(j.started_at)) / 1000;
  const left = (elapsed / j.done) * (j.total - j.done);
  return left < 60
    ? `${Math.round(left)} 秒`
    : left < 3600
      ? `${Math.round(left / 60)} 分钟`
      : `${(left / 3600).toFixed(1)} 小时`;
}

function StatusBadge({ s }: { s: Job["status"] }) {
  return (
    <span
      className={`rounded-full px-2 py-0.5 text-xs font-medium ${STATUS[s].cls}`}
    >
      {STATUS[s].label}
    </span>
  );
}

function Stat({
  label,
  value,
  sub,
  warn,
  action,
}: {
  label: string;
  value: string;
  sub: string;
  warn?: boolean;
  action?: React.ReactNode;
}) {
  return (
    <Card className="gap-1 py-4">
      <CardContent className="space-y-1 px-4">
        <div className="flex items-center justify-between gap-2 text-xs text-muted-foreground">
          {label}
          {action}
        </div>
        <div
          className={`text-2xl font-semibold tabular-nums tracking-tight ${warn ? "text-[var(--warning)]" : ""}`}
        >
          {value}
        </div>
        <div className="truncate text-xs text-muted-foreground" title={sub}>
          {sub}
        </div>
      </CardContent>
    </Card>
  );
}

function Meter({
  icon,
  label,
  value,
  detail,
  bad,
}: {
  icon: React.ReactNode;
  label: string;
  value: number;
  detail: string;
  bad?: boolean;
}) {
  const v = Math.max(0, Math.min(100, value));
  const tone =
    bad || v > 90
      ? "[&>div]:bg-destructive"
      : v > 75
        ? "[&>div]:bg-[var(--warning)]"
        : "";
  return (
    <Card className="gap-2 py-4">
      <CardContent className="space-y-2 px-4">
        <div className="flex items-center gap-2 text-sm">
          {icon}
          {label}
          <span className="ml-auto font-semibold tabular-nums">
            {bad ? "—" : `${v.toFixed(0)}%`}
          </span>
        </div>
        <Progress value={bad ? 0 : v} className={`h-1.5 ${tone}`} />
        <div className="truncate text-xs text-muted-foreground" title={detail}>
          {detail}
        </div>
      </CardContent>
    </Card>
  );
}

function JobLog({ id, onClose }: { id: number | null; onClose: () => void }) {
  const [job, setJob] = useState<Job | null>(null);
  useEffect(() => {
    if (id == null) return setJob(null);
    let alive = true;
    const load = async () => {
      const j = await api<Job>(`/api/admin/jobs/${id}`).catch(() => null);
      if (alive) setJob(j);
    };
    load();
    const t = setInterval(load, 2000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [id]);
  return (
    <Dialog open={id != null} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[85dvh] sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            任务 #{id}{" "}
            {job && (
              <>
                · {KIND[job.kind]?.label} <StatusBadge s={job.status} />
              </>
            )}
          </DialogTitle>
        </DialogHeader>
        {job && (
          <div className="space-y-3 overflow-auto text-sm">
            <div className="grid grid-cols-2 gap-2 text-xs text-muted-foreground sm:grid-cols-4">
              <span>
                进度 {fmt.num(job.done)}
                {job.total ? `/${fmt.num(job.total)}` : ""}
              </span>
              <span>耗时 {fmt.duration(job.started_at, job.finished_at)}</span>
              <span>发起 {job.created_by}</span>
              <span>参数 {JSON.stringify(job.params)}</span>
            </div>
            {job.error && (
              <p className="rounded-md bg-destructive/10 p-2 text-destructive">
                {job.error}
              </p>
            )}
            <pre className="max-h-[50dvh] overflow-auto whitespace-pre-wrap rounded-md bg-muted p-3 font-mono text-xs leading-relaxed">
              {job.log || "（暂无日志）"}
            </pre>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
