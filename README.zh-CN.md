<div align="center">

# BoothDex

**自托管的 BOOTH VRChat 素材搜索引擎，使用 Rust 和 Next.js 编写。**

**[部署指南](docs/DEPLOY.md)**（英文） / **[API](#api)** / **[许可证](#许可证)**

[English](README.md) / 简体中文 / [日本語](README.ja.md)

[功能](#功能) / [快速开始](#快速开始) / [架构](#架构) / [开发](#开发)

</div>

> **早期阶段软件。** 各服务与 Linux 容器部署均已跑通，但尚未作为公开多用户站点长期运行，可能仍有细节有待打磨。

BoothDex 收录 [BOOTH](https://booth.pm) 上出售的 VRChat 商品，支持按适配模型、分类、标签、功能、价格等条件筛选。
项目起初作为 VRCatalogue（已于 2026-11-01 关闭）的后继方案开发，保留了兼容的搜索语法。本项目为独立开源项目，与
pixiv、BOOTH 及 VRCatalogue 均无关联；商品数据与图片归原作者所有，每件商品均直接链接回其 BOOTH 页面。

## 功能

- **搜索**：在约 20 万件商品中按适配模型、智能分类、BOOTH 分类、标签（包含与排除）、功能（MA、lilToon、Poiyomi、
  VRCFury）、价格、上架时间、免费商品及内容分级进行多维筛选，耗时仅数毫秒。日文全文检索基于 Lindera（IPADIC）；可选的语义搜索支持跨语言按语义查找（如搜索“哥特裙”即可匹配ゴスロリ连衣裙），并与关键词结果融合排序。
- **浏览**：按性别分组的模型本体视图；内置完整 BOOTH 图集、变体、多语言标题与原图下载的商品详情面板；支持 `Ctrl+K`
  全局检索商品、模型、店铺与标签。
- **账户**：收藏夹、最近已看（在结果中自动变暗标记）、保存搜索条件、记住常用模型，以及任意筛选条件的 RSS 订阅。
- **界面**：支持简体中文、英语、日语、韩语界面；商品标题支持原文或译文切换；支持货币换算、浅色/深色主题及移动端优先布局。
- **抓取**：`worker` 每小时增量获取 BOOTH 新上架商品，每日全量巡检价格与下架状态，并定期刷新热门商品。
- **AI 增强**（可选，兼容任意 OpenAI API，可配置思考强度）：自动为新商品提取智能分类、内容分级、支持功能及四语标题翻译（中/英/日/韩），通过名称匹配关联适配模型，并翻译分类与标签名称；语义搜索所需的向量同样由同一接口生成。
- **图片**：内置代理将 BOOTH 图片实时转码为 WebP 并多级缓存；已下架商品自动回退至存档图片。
- **管理**：通过 SSE 实时查看任务进度，管理定时计划、资源占用、AI 配置及重置用户密码。

不提供独立商品详情页、历史价格曲线、第三方社交登录及找回密码邮件（由管理员重置密码）。商品简介和变体名保留原文，仅翻译标题以服务跨语言检索。

## 快速开始

完整配置、反向代理示例及备份方案请参阅[部署指南](docs/DEPLOY.md)。生产环境通过 Docker 或 Podman Compose 运行
PostgreSQL 17（`pgvector`）、三个 Rust 服务与 Next.js 前端，仅对外暴露单一端口（`WEB_LISTEN`，默认 `3100`）供反向代理接入；建议使用 **6 GB 及以上内存**的 Linux 服务器（推荐 8 GB，或 4 GB 配合 Swap）。

```mermaid
flowchart LR
  env["1. 下载 compose.yaml 与 .env<br/>（填写密码与 SITE_URL）"] --> data{"2. 准备初始数据"}

  data -- "使用数据包（约 5 分钟）" --> bundle["解压 images.tar<br/>pg_restore 导入 catalogue.dump"]
  data -- "空库抓取（约 2–3 天）" --> crawl["自动写入分类与约 380 个模型<br/>运行首次全量 incremental 抓取"]

  bundle --> up["3. docker compose up -d"]
  crawl --> up
  up --> admin["4. 注册账号并写入 ADMIN_USERNAMES<br/>在 /admin 配置可选 AI 服务"]
```

新建数据库会自动初始化分类树、智能标签和约 380 个模型本体。若想跳过耗时数天的首次全量抓取（参见[无数据包首次启动](docs/DEPLOY.md#starting-without-a-data-bundle)），可直接恢复[已发布的数据包](https://github.com/JohnsonRan/BoothDex/releases/tag/data-2026-11-04)（2026-11-04 快照，含约 21.7 万件商品及下架存档图片，约 2.1 GB）：

```sh
mkdir -p /srv/boothdex && cd /srv/boothdex
curl -fLO https://raw.githubusercontent.com/JohnsonRan/BoothDex/master/deploy/compose.yaml
curl -fLo .env https://raw.githubusercontent.com/JohnsonRan/BoothDex/master/deploy/.env.example
$EDITOR .env    # 配置 POSTGRES_PASSWORD、SITE_URL、WEB_LISTEN，使用数据包时还需设置 ARCHIVE_IMAGES_DIR
docker compose pull && docker compose up -d
```

## 架构

### 服务拓扑

```mermaid
flowchart LR
  browser["浏览器 / RSS 阅读器"] --> web["web (Next.js :3100)<br/>前端界面与 /admin"]

  subgraph stack["BoothDex 服务栈"]
    web -- "/api/*" --> api["api (Rust, Axum :3000)<br/>Roaring 位图 · Tantivy"]
    web -- "/img/*" --> imgproxy["imgproxy (Rust :3001)<br/>foyer WebP 缓存"]
    worker["worker (Rust)<br/>booth · ai · embed 三通道"]
    pg[("PostgreSQL 17<br/>+ pgvector")]

    api --> pg
    worker --> pg
    api --- idx[("INDEX_DIR<br/>(Tantivy 磁盘索引)")]
    imgproxy --- cache[("imgcache 与<br/>存档 WebP 图片")]
  end

  imgproxy --> pximg["booth.pximg.net"]
  worker --> booth["booth.pm"]
  worker -. "分类翻译 / 向量化" .-> ai["OpenAI 兼容 API"]
  api -. "查询词向量化" .-> ai
```

### 混合搜索与数据更新流程

```mermaid
flowchart TD
  subgraph query["混合搜索流程 (/api/search)"]
    q["查询词 + 多维筛选参数"] --> parse["识别模型名与分类短语"]
    parse --> kw["Lindera (IPADIC) + Tantivy<br/>与 Roaring 内存位图筛选"]
    parse -. "启用语义搜索时" .-> embq["查询词向量<br/>（内存 → 数据库缓存 → AI 接口，2.5 秒超时回退）"]
    embq -.-> ann["pgvector HNSW 近邻检索"]
    kw --> merge["融合排序与分面统计"]
    ann -.-> merge
    merge --> hydrate["从 PostgreSQL 读取当前页商品详情"]
  end

  subgraph ingest["后台任务通道与无锁热重载"]
    crawl_job["booth 通道：<br/>incremental · sweep · refresh"] --> dbw[("PostgreSQL")]
    ai_job["ai 通道：<br/>enrich（每批 200 条自动续跑）· localize"] --> dbw
    ai_job -. "队列完成后触发" .-> emb_job["embed 通道：<br/>1024 维 halfvec 向量"]
    emb_job --> dbw
    dbw -- "递增 catalog_meta.version" --> snap["api 后台构建新快照<br/>并通过 ArcSwap 原子切换"]
  end
```

### 目录结构

| 路径 | 说明 |
|---|---|
| `crates/core` | 公共类型、BOOTH 客户端及 `migrations/` 中的 PostgreSQL 表结构（服务启动时自动执行迁移） |
| `crates/api` | 搜索 API。筛选使用内存 Roaring 位图；Tantivy 文本索引持久化在磁盘（`INDEX_DIR`）并在重启间复用；分页详情从 PostgreSQL 读取 |
| `crates/worker` | 分三通道并发执行的后台任务器：`incremental`（增量上新）、`sweep`（全量巡检）、`details`、`refresh`、`enrich`、`localize` 与 `embed`（AI）。`worker run` 为守护进程，`worker once <kind> [params-json]` 单次执行指定任务 |
| `crates/imgproxy` | `/b/{size}/...` 将 BOOTH 图片转码为 WebP 并缓存；`/v/{sha1}.webp` 提供下架商品存档图片 |
| `web` | Next.js 前端站点与 `/admin` 管理后台 |
| `deploy` | 容器构建文件、`docker-bake.hcl`、`compose.yaml` 与 `.env.example` |
| `scripts` | 本地开发启停脚本（Windows / Git Bash）、数据包打包校验脚本与模拟 AI 服务 |

在 32 线程机器上面对 21.6 万件商品时，API 内部检索耗时 1–6 毫秒，单次请求端到端耗时 5–40 毫秒，32 并发下吞吐约 550 请求/秒。在 Linux 容器环境中，`api` 常驻内存约 0.4–1 GB（mmap 映射的 Tantivy 索引另计在页缓存中），热重载双份目录快照时峰值约 1.8 GB（容器限制 `4g`）。

## 开发

环境要求：Rust（stable）、[Bun](https://bun.sh)，以及带 `pgvector` 扩展的 PostgreSQL 17（运行于 Podman 或 Docker）。

```sh
podman run -d --name boothdex-pg -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=catalogue \
  -p 5432:5432 -v boothdex-pgdata:/var/lib/postgresql/data docker.io/pgvector/pgvector:pg17

cargo build --release            # 首次构建会下载 IPADIC 词典；若网络失败重试即可
(cd web && bun install)

export DATABASE_URL=postgres://postgres:dev@localhost:5432/catalogue COOKIE_SECURE=0 ADMIN_USERNAMES=admin
target/release/api &             # :3000
target/release/imgproxy &        # :3001
target/release/worker run &
(cd web && bun run dev)          # :3100，将 /api 与 /img 转发至上述服务
```

注册账户 `admin`，打开 `http://localhost:3100/admin`，再抓取几页数据即可开始调试：

```sh
target/release/worker once incremental '{"max_pages": 3, "full": true}'
```

- **Windows（Git Bash）**：使用 `scripts/dev-up.sh`、`scripts/dev-down.sh` 和 `scripts/dev-restart.sh` 可在后台一键启停全部四个服务，日志输出至 `<service>.log`。
- **模拟 AI 服务**：没有 API 密钥时，运行 `bun scripts/mock-ai.ts` 并在管理后台将服务地址设为 `http://127.0.0.1:3555/v1` 即可联调 AI 任务。

运行测试与静态检查：

```sh
cargo test
cd web && bun test src/lib && bunx tsc --noEmit && bunx biome check src
```

## API

`/api/search` 接受与 VRCatalogue 兼容的查询参数：`v q sort dir page pageSize adult avatars authors tags extags
btags exbtags sc scp bc if free pmin pmax range from to asup gender fields`。其余公开接口包括：`/api/avatars`、`/api/authors`、
`/api/tags`、`/api/booth-tags`（均支持 `?q=&offset=&limit=`）、`/api/categories`、`/api/items?ids=`、`/api/palette?q=`
以及 `/api/rss?<搜索参数>`。

与 VRCatalogue 的差异：功能（Features）分面计数会联动当前选中的分类；“热门”（Trending）排序按点赞数随时间衰减计算；`range` 额外支持 `7d`、`30d` 与 `90d`；`fields=card` 可选返回卡片精简记录（bio、variations、booth_tags、linked_avatars、categories 与 smart_price_reason 置空，translations 仅保留标题，booth_images/media 最多 2 张；默认响应保持完整；完整记录通过 `/api/items?ids=` 获取）。

## 致谢

- VRCatalogue：原站点、其搜索交互模型，以及本项目起步时沿用的分类体系
- [BOOTH](https://booth.pm)：商品数据与图片源站
- [Tantivy](https://github.com/quickwit-oss/tantivy) 与 [Lindera](https://github.com/lindera/lindera)：全文检索引擎与日文分词
- [roaring-rs](https://github.com/RoaringBitmap/roaring-rs)：内存筛选位图
- [foyer](https://github.com/foyer-rs/foyer)：混合图片缓存
- [shadcn/ui](https://ui.shadcn.com)：前端界面组件

## 许可证

采用 [AGPL-3.0-or-later](LICENSE) 许可证。若你将修改后的版本作为公开网络服务运行，必须公开相应的源代码修改。

```text
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (c) 2026, BoothDex contributors
```
