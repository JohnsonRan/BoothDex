<div align="center">

<img src="web/src/app/icon.svg" width="104" alt="BoothDex">

# BoothDex

**自托管的 BOOTH VRChat 素材搜索引擎，使用 Rust 和 Next.js 编写。**

**[部署指南](docs/DEPLOY.md)**（英文） / **[API](#api)** / **[许可证](#许可证)**

[English](README.md) / 简体中文 / [日本語](README.ja.md)

[快速开始](#快速开始) / [功能](#功能) / [文档](#文档) / [架构](#架构) / [开发](#开发)

</div>

BoothDex 收录 [BOOTH](https://booth.pm) 上出售的 VRChat 商品，支持按适配的虚拟形象、分类、标签、功能、价格等条件筛选。
项目起初作为 VRCatalogue（已于 2026-11-01 关闭）的后继方案开发，保留了兼容的搜索语法。本项目为独立开源项目，与
pixiv、BOOTH 及 VRCatalogue 均无关联；商品数据与图片归原作者所有，每件商品均直接链接回其 BOOTH 页面。

![首页（分类：头发）：左侧筛选栏、虚拟形象本体轮播与商品网格](docs/screenshots/zh-CN/home.webp)

## 快速开始

BoothDex 通过 Docker 或 Podman Compose 运行 PostgreSQL 17（`pgvector`）、三个 Rust 服务与 Next.js 前端。预构建镜像支持 `linux/amd64` 和 `linux/arm64`。服务器需有 **4 GB 及以上内存**和 2 GB 交换空间（推荐 6 GB 内存）。整个服务栈只暴露一个端口（`WEB_LISTEN`，默认 `127.0.0.1:3100`），供反向代理接入。

使用已发布的数据包安装。数据包是 2026-11-04 的快照，含约 21.7 万件商品及下架商品的存档图片（约 2.1 GB）。先按[部署指南](docs/DEPLOY.md#optional-download-the-data-bundle)（英文）下载，然后执行：

```sh
mkdir -p /srv/boothdex && cd /srv/boothdex
curl -fLO https://raw.githubusercontent.com/JohnsonRan/BoothDex/master/deploy/compose.yaml
curl -fLo .env https://raw.githubusercontent.com/JohnsonRan/BoothDex/master/deploy/.env.example
$EDITOR .env    # 配置 POSTGRES_PASSWORD、SITE_URL、WEB_LISTEN、CONTACT_EMAIL，使用数据包时还需设置 ARCHIVE_IMAGES_DIR
docker compose pull && docker compose up -d postgres
until docker compose exec -T postgres pg_isready -h 127.0.0.1 -U catalogue -d catalogue; do sleep 2; done
docker compose exec -T postgres pg_restore -U catalogue -d catalogue --no-owner --role=catalogue \
  < /srv/boothdex/bundle/catalogue.dump
docker compose up -d
```

请在 `pg_restore` 无错误完成后再执行最后一行启动服务栈，因为 `api` 会直接迁移它找到的数据库。不使用数据包时，省略 `pg_restore` 命令，改为抓取 BOOTH，需要 2–3 天（参见[无数据包首次启动](docs/DEPLOY.md#starting-without-a-data-bundle)）。新建数据库已包含分类树、智能标签和约 600 个虚拟形象本体。

然后通过反向代理打开网站，注册账号，把该用户名写入 `.env` 的 `ADMIN_USERNAMES`，再运行 `docker compose up -d api`。管理后台位于 `/admin`。

## 功能

搜索（[详情](docs/SEARCH.md)，英文）：

- 在约 20 万件商品中按适配的虚拟形象、智能分类、BOOTH 分类、标签（包含与排除）、功能（MA、lilToon、Poiyomi、VRCFury）、价格、上架时间、免费商品及内容分级筛选，耗时仅数毫秒。
- 搜索框给出最近搜索、虚拟形象、商品、店铺与标签建议，并把每个搜索词显示为可单独删除的词条。
- 搜索 BOOTH 商品链接或商品编号，该商品会排在最前。
- 日文和中文全文检索；不区分全角与半角、平假名与片假名；英文单词允许 1 处拼写错误。
- BOOTH 标签也按英、日、韩、中译词匹配，因此能找到以其他语言打标签的商品。
- 可选的语义搜索支持跨语言按语义查找，并提供最多 4 个相关搜索。

浏览与账户：

- 在虚拟形象轮播中选择虚拟形象本体，即可列出适配它的商品；“虚拟形象本体”视图列出虚拟形象本身，并可按性别筛选。
- 滚动一屏后出现“返回顶部”按钮，筛选条件和结果保持不变。
- 商品在对话框中打开，包含完整 BOOTH 图集、变体、标题翻译与原图下载。
- 收藏夹、最近已看、记住常用的虚拟形象，以及可重命名、可通过 RSS 订阅的已保存搜索。
- 在设置中修改显示名称和密码；修改密码后，其他设备会退出登录。
- 顶栏的留言板无需账号即可留言（昵称、最多 500 个字符，并需通过 Cloudflare Turnstile 验证）。管理员可隐藏或删除留言，也可将留言标为已完成或不计划处理。
- 管理员可要求登录和注册也通过 Turnstile 验证，并对未通过验证的访客施加更严格的限制（[Turnstile](docs/SERVICES.md#cloudflare-turnstile)，英文）。

<details>
<summary>输入“manuka”时的建议：先列出虚拟形象本体，再列出匹配的商品</summary>

![搜索框下方按虚拟形象和商品分组的搜索建议](docs/screenshots/zh-CN/search.webp)

</details>

<details>
<summary>商品详情在结果上方打开</summary>

![商品对话框：图集、翻译后的标题与“在 Booth 购买”按钮](docs/screenshots/zh-CN/item.webp)

</details>

外观与语言：

- 界面基于 Meta 的 [Astryx](https://github.com/meta/astryx) 设计系统构建。
- 可选浅色、深色或跟随系统；深色模式下可开启纯黑模式；还可减少动效，关闭时跟随系统设置。这些设置随账户同步。
- 支持简体中文、英语、日语、韩语界面；商品标题支持原文或译文切换；支持货币换算，手机上同样可用。

<details>
<summary>在设置中选择颜色模式</summary>

![设置中的外观页：主题模式、纯黑模式与顶部筛选](docs/screenshots/zh-CN/settings.webp)

</details>

<details>
<summary>手机上的首页（英文界面）</summary>

<p align="center"><img src="docs/screenshots/phone.webp" width="390" alt="手机上的首页，筛选条件收进 +8 按钮"></p>

</details>

抓取、AI 与图片（[详情](docs/WORKER.md)，英文）：

- `worker` 每小时增量获取 BOOTH 新上架商品，每日全量巡检并标记下架商品，并定期刷新热门商品。
- 可选的 AI 增强（兼容任意 OpenAI API）：为商品提取智能分类、内容分级、支持功能，关联适配的虚拟形象，翻译标题，并翻译分类与标签名称。语义搜索所需的向量由同一接口生成。
- 内置代理将 BOOTH 图片和店铺图标转码为 AVIF 并缓存；已下架商品自动回退至存档图片。

管理后台：

- `/admin` 管理后台实时显示运行状态和 30 天趋势图，可运行、停止和定时执行任务，并查看实时日志和历史，包括最近 30 天内未被之后同类运行补救的失败运行。
- 列出 AI 增强放弃处理的商品，可逐个或批量重试。
- 配置 AI 服务，开放或关闭注册。
- 查找用户，查看其登录设备，让某台设备退出登录，禁用账号或重置密码。
- 管理后台支持简体中文、英语和日语。

<details>
<summary>管理后台概览</summary>

![管理后台概览：worker 状态、索引与活动卡片及下方的趋势图](docs/screenshots/zh-CN/admin.webp)

</details>

不提供独立商品详情页、历史价格曲线、第三方社交登录及找回密码邮件（由管理员重置密码）。商品简介和变体名保留原文，仅翻译标题和 BOOTH 标签。

## 文档

| 页面 | 读者目标 |
|---|---|
| [部署指南](docs/DEPLOY.md) | 安装、配置、更新和备份服务器 |
| [搜索行为](docs/SEARCH.md) | 了解搜索如何处理搜索词 |
| [API 参考](docs/API.md) | 调用 HTTP API |
| [后台任务与 AI 增强](docs/WORKER.md) | 了解 worker 运行哪些任务、AI 服务做什么 |
| [服务行为](docs/SERVICES.md) | 了解图片代理、速率限制、缓存和 Turnstile 的行为 |
| [实测数据](docs/MEASUREMENTS.md) | 查阅文档引用的数值 |

以上页面仅提供英文版本。

## 架构

### 服务拓扑

```mermaid
flowchart LR
  browser["浏览器 / RSS 阅读器"] --> web["web (Next.js :3100)<br/>前端界面与 /admin"]

  subgraph stack["BoothDex 服务栈"]
    web -- "/api/*" --> api["api (Rust, Axum :3000)<br/>Roaring 位图 · Tantivy"]
    web -- "/img/*" --> imgproxy["imgproxy (Rust :3001)<br/>foyer AVIF 缓存"]
    worker["worker (Rust)<br/>booth · ai · embed 三通道"]
    pg[("PostgreSQL 17<br/>+ pgvector")]

    api --> pg
    worker --> pg
    api --- idx[("INDEX_DIR<br/>(Tantivy 磁盘索引)")]
    imgproxy --- cache[("imgcache 与<br/>存档 WebP 图片")]
  end

  imgproxy --> pximg["booth.pximg.net"]
  imgproxy --> asset["asset.booth.pm"]
  imgproxy --> factory["factory.pixiv.net"]
  worker --> booth["booth.pm"]
  worker -. "分类翻译 / 向量化" .-> ai["OpenAI 兼容 API"]
  api -. "查询词向量化" .-> ai
```

### 混合搜索与数据更新流程

```mermaid
flowchart TD
  subgraph query["混合搜索流程 (/api/search)"]
    q["查询词 + 多维筛选参数"] --> parse["识别商品链接或编号、虚拟形象名与分类短语"]
    parse --> kw["Lindera (IPADIC, CC-CEDICT) + Tantivy<br/>+ 译为查询的标签<br/>与 Roaring 内存位图筛选"]
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
| `crates/limit` | 反向代理后的客户端 IP 与按 IP 的令牌桶，由 `api` 和 `imgproxy` 共用 |
| `crates/api` | 搜索 API。筛选使用内存 Roaring 位图；Tantivy 文本索引持久化在磁盘（`INDEX_DIR`）并在重启间复用；分页详情从 PostgreSQL 读取 |
| `crates/worker` | 分三通道并发执行的后台任务器：`incremental`（增量上新）、`sweep`（全量巡检）、`details`、`refresh`、`enrich`、`localize` 与 `embed`（AI）。`worker run` 为守护进程，`worker once <kind> [params-json]` 单次执行指定任务 |
| `crates/imgproxy` | `/b/{size}/...`（商品图片）、`/u/{size}/...`（店铺图标）、`/a/{file}`（BOOTH 静态图片）和 `/f/{size}/{uuid}/{file}`（pixiv Factory 图片，由 imgproxy 缩小到该尺寸）转码为 AVIF 并缓存；`/v/{sha1}.webp` 提供下架商品存档图片 |
| `web` | Next.js 前端站点与 `/admin` 管理后台 |
| `deploy` | 容器构建文件、`docker-bake.hcl`、`compose.yaml` 与 `.env.example` |
| `scripts` | 数据包打包校验脚本与模拟 AI 服务 |

实测的响应时间与内存占用见[实测数据](docs/MEASUREMENTS.md)（英文）。

## 开发

环境要求：Rust（stable）、[Bun](https://bun.sh)，以及带 `pgvector` 扩展的 PostgreSQL 17（运行于 Podman 或 Docker）。

```sh
podman run -d --name boothdex-pg -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=catalogue \
  -p 5432:5432 -v boothdex-pgdata:/var/lib/postgresql/data docker.io/pgvector/pgvector:pg17

cargo build --release            # 首次构建会下载 IPADIC 和 CC-CEDICT 词典；若网络失败重试即可
# 容器镜像改用 libaom 编码 AVIF：--features imgproxy/libavif（需要 libavif-dev 和 pkg-config）。
# 不加时使用 rav1e；已安装 nasm 时可加上 --features imgproxy/asm，编码速度约为原来的三倍。
(cd web && bun install)

export DATABASE_URL=postgres://postgres:dev@127.0.0.1:5432/catalogue COOKIE_SECURE=0
target/release/api &             # :3000
target/release/imgproxy &        # :3001
target/release/worker run &
(cd web && bun run dev)          # :3100，将 /api 与 /img 转发至上述服务
```

在 `http://localhost:3100` 注册账户 `admin`，停止 `api`，再带上该用户名重新启动。API 拒绝注册 `ADMIN_USERNAMES` 中列出的用户名，因此请在账户创建后再将其写入：

```sh
ADMIN_USERNAMES=admin target/release/api &   # :3000，先停止之前的 api
```

打开 `http://localhost:3100/admin`，再抓取几页数据即可开始调试：

```sh
target/release/worker once incremental '{"max_pages": 3, "full": true}'
```

- **模拟 AI 服务**：没有 API 密钥时，运行 `bun scripts/mock-ai.ts` 并在管理后台将服务地址设为 `http://127.0.0.1:3555/v1`，同时填入任意 API 密钥一起保存，即可联调 AI 任务。

运行测试与静态检查：

```sh
cargo test --workspace -- --include-ignored && CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets -- -D warnings
python3 scripts/check-requirements.py && python3 scripts/test_check_requirements.py
cd web && bun test && bunx next typegen && bunx tsc --noEmit && bunx biome check
```

## API

`/api/search` 接受与 VRCatalogue 兼容的查询参数：`v q sort dir page pageSize adult avatars authors tags extags btags exbtags sc scp bc if free pmin pmax range from to asup gender fields`。其余公开接口包括：`/api/avatars`、`/api/authors`、`/api/tags`、`/api/booth-tags`（均支持 `?q=&offset=&limit=`）、`/api/categories`、`/api/items?ids=`、`/api/palette?q=`、`/api/related?<搜索参数>` 以及 `/api/rss?<搜索参数>`。与 VRCatalogue 的差异、新增参数和响应字段见 [API 参考](docs/API.md)（英文）。

## 致谢

- VRCatalogue：原站点、其搜索交互模型，以及本项目起步时沿用的分类体系
- [BOOTH](https://booth.pm)：商品数据与图片源站
- [Tantivy](https://github.com/quickwit-oss/tantivy) 与 [Lindera](https://github.com/lindera/lindera)：全文检索引擎与日文、中文分词
- [CC-CEDICT](https://www.mdbg.net/chinese/dictionary?page=cedict)（[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)）：内置于 `api` 的中文词典（经由 Lindera 的 CC-CEDICT-MeCab），用于中文标题分词
- IPADIC（奈良先端科学技术大学院大学）：内置于 `api` 的日文词典。两部词典的许可文本见 [deploy/licenses](deploy/licenses)，`api` 镜像也将其放在 `/usr/share/doc/boothdex-api/`
- [roaring-rs](https://github.com/RoaringBitmap/roaring-rs)：内存筛选位图
- [foyer](https://github.com/foyer-rs/foyer)：混合图片缓存
- [Astryx](https://github.com/meta/astryx)：设计系统、界面组件与风格

## 许可证

采用 [AGPL-3.0-or-later](LICENSE) 许可证。若你将修改后的版本作为公开网络服务运行，必须公开相应的源代码修改。

```text
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (c) 2026, BoothDex contributors
```
