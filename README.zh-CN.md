<div align="center">

# BoothDex

**自托管的 BOOTH VRChat 素材搜索引擎，使用 Rust 和 Next.js 编写。**

**[部署指南](docs/DEPLOY.md)**(英文) / **[API](#api)** / **[许可证](#许可证)**

[English](README.md) / 简体中文

[功能](#功能) / [快速开始](#快速开始) / [架构](#架构) / [开发](#开发)

</div>

> **早期阶段，尚未作为公开网站运行。** 各部分都已在开发机上跑通，但完整的容器部署还没有在真实服务器上运行过，
> 可能还有不完善的地方。

BoothDex 收录 [BOOTH](https://booth.pm) 上出售的 VRChat 商品，可以按适配的模型、分类、标签、价格等条件筛选。
它最初是 VRCatalogue(已于 2026-11-01 关闭)的后继项目，沿用了其搜索语法。本项目是独立项目，与 pixiv、BOOTH 和
VRCatalogue 均无关联。商品数据和图片归其作者所有，每个商品都链接回它在 BOOTH 上的页面。

## 功能

- 搜索:在约 20 万个商品中按适配模型、智能分类、BOOTH 分类、标签(包含和排除)、功能(MA、lilToon、Poiyomi、
  VRCFury)、价格、发布时间、免费商品和成人内容模式筛选，耗时只有几毫秒。日文全文搜索使用 Lindera(IPADIC);可选的语义搜索按意思跨语言查找(搜"哥特裙"能找到ゴスロリ连衣裙)，并与关键词结果合并排序。
- 浏览:按性别分组的模型视图;商品详情面板，含完整的 BOOTH 图集、变体、翻译和图片下载;用 Ctrl+K 搜索商品、
  模型、作者和标签。
- 账户:收藏、最近已看(在结果中变暗显示)、保存的搜索、记住你的模型，以及任意搜索的 RSS 订阅。
- 界面:中文、英语、日语和韩语;商品文字可显示原文或译文;货币换算显示;浅色和深色主题;移动端优先的布局。
- 抓取:worker 每小时从 BOOTH 获取新商品，每天巡检完整列表，定期重新检查热门商品，并标记已下架的商品。
- AI 处理(可选，支持任何兼容 OpenAI 的 API，可配置思考等级):为新商品生成分类、内容分级、功能和标题翻译(中英日韩)，按名称
  匹配适配模型，并把分类和标签名翻译成中文。语义搜索的向量也由同一个服务生成。
- 图片:代理把 BOOTH 图片转成 WebP 并缓存;已下架商品的图片使用存档副本。
- 管理:实时查看任务进度，管理定时任务、资源占用、AI 设置，以及重置用户密码。

不提供独立的商品页面、价格历史、第三方账户登录和找回密码邮件(由管理员重置密码)。商品简介和变体名不翻译；
标题会翻译，用于跨语言搜索。

## 快速开始

按照[部署指南](docs/DEPLOY.md)操作。它通过 Docker 或 Podman compose 运行 PostgreSQL、三个 Rust 服务和网站，
对外只开放一个端口，供你自己的反向代理接入，需要一台有 4 GB 内存的 Linux 服务器。

新建的数据库已经包含分类树、智能标签和约 380 个模型本体。商品需要从 BOOTH 抓取:先完整抓取一次
([没有数据包时的首次启动](docs/DEPLOY.md#starting-without-a-data-bundle))，之后由每小时的抓取任务保持更新。
如果想跳过首次抓取，可以恢复[已发布的数据包](https://github.com/JohnsonRan/BoothDex/releases/tag/data-2026-10-31)
(2026-10-31 的快照，约 21.6 万个商品及存档图片，约 1.4 GB)。

## 架构

```
浏览器 ─► web (Next.js) ─┬─ /api/*  ─► api (Rust, Axum) ──► PostgreSQL
                         └─ /img/*  ─► imgproxy (Rust) ──► booth.pximg.net、存档图片
         worker (Rust) ──► booth.pm、AI 服务、PostgreSQL
```

| 路径 | 内容 |
|---|---|
| `crates/core` | 公共代码;`migrations/` 中的 PostgreSQL 表结构，服务启动时自动应用 |
| `crates/api` | 搜索 API。筛选用内存中的 roaring 位图;Tantivy 文本索引放在磁盘上(`INDEX_DIR`)，目录不变时直接复用;分页数据从 PostgreSQL 读取 |
| `crates/worker` | 任务执行器:`incremental`(新商品)、`sweep`(完整列表)、`details`、`refresh`、`enrich`、`localize` 和 `embed`(AI)。`worker run` 以守护进程运行;`worker once <kind> [params-json]` 立即运行一个任务 |
| `crates/imgproxy` | `/b/{size}/...` 把 BOOTH 图片转成 WebP 提供;`/v/{sha1}.webp` 提供存档图片 |
| `crates/importer` | 一次性导入 VRCatalogue 存档目录 |
| `web` | Next.js 网站和 `/admin` 管理后台 |
| `deploy` | 容器镜像和 compose 配置 |
| `scripts` | 开发环境启动脚本(Windows 和 Git Bash)、数据包工具和模拟 AI 服务 |

在一台 32 线程的开发机上，对 21.6 万个商品搜索时，API 内部耗时 1–6 毫秒，每个请求 5–40 毫秒;32 个客户端并发时
约每秒 550 个请求。API 空闲时占用约 400 MB 内存，加载数据时峰值约 1.2 GB。

## 开发

需要:Rust(stable)、[Bun](https://bun.sh)，以及带 pgvector 的 PostgreSQL 17(例如运行在 Podman 或 Docker 中)。

```sh
podman run -d --name boothdex-pg -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=catalogue \
  -p 5432:5432 -v boothdex-pgdata:/var/lib/postgresql/data docker.io/pgvector/pgvector:pg17

cargo build --release            # 首次构建会下载 IPADIC 词典;下载失败时重新运行即可
(cd web && bun install)

export DATABASE_URL=postgres://postgres:dev@localhost:5432/catalogue COOKIE_SECURE=0 ADMIN_USERNAMES=admin
target/release/api &             # :3000
target/release/imgproxy &        # :3001
target/release/worker run &
(cd web && bun run dev)          # :3100，把 /api 和 /img 转发给上面的服务
```

注册账户 `admin`，打开 http://localhost:3100/admin，再抓取几页获取商品:
`target/release/worker once incremental '{"max_pages": 3, "full": true}'`。

在 Windows 上使用 Git Bash 时，可以用 `scripts/dev-up.sh`、`scripts/dev-down.sh` 和 `scripts/dev-restart.sh`
在后台启动和停止全部四个服务，日志写入 `<service>.log`。

没有 API 密钥时，可以运行 `bun scripts/mock-ai.ts`，并在管理后台把 AI 服务地址设为 `http://127.0.0.1:3555/v1`，
用来试用 AI 任务。

如果有 VRCatalogue API 格式的存档(`items/`、`avatars/`、`dict/`、`labels/`)，可以用
`DATABASE_URL=... cargo run --release -p importer -- <存档目录>` 导入。可以重复运行;21.5 万个商品约需 90 秒。

测试:

```sh
cargo test
cd web && bun test src/lib && bunx tsc --noEmit && bunx biome check src
```

## API

`/api/search` 接受与 VRCatalogue 兼容的查询参数:`v q sort dir page pageSize adult avatars authors tags extags
btags exbtags sc scp bc if free pmin pmax range from to asup gender`。其他接口:`/api/avatars`、`/api/authors`、
`/api/tags` 和 `/api/booth-tags`(`?q=&offset=&limit=`)、`/api/categories`、`/api/items?ids=`、`/api/palette?q=`
和 `/api/rss?<搜索参数>`。

与 VRCatalogue 的不同之处:功能筛选的计数会考虑当前选中的分类;"热门"排序按点赞数随时间衰减计算;`range` 还支持
`7d`、`30d` 和 `90d`。

## 致谢

- VRCatalogue:原网站、它的搜索模型，以及本项目起步时使用的分类体系
- [BOOTH](https://booth.pm):商品数据和图片
- [Tantivy](https://github.com/quickwit-oss/tantivy) 和 [Lindera](https://github.com/lindera/lindera):文本搜索
- [roaring-rs](https://github.com/RoaringBitmap/roaring-rs):筛选位图
- [foyer](https://github.com/foyer-rs/foyer):图片缓存
- [shadcn/ui](https://ui.shadcn.com):界面组件
- [MiSans](https://hyperos.mi.com/font/):小米系统界面字体

## 许可证

[AGPL-3.0-or-later](LICENSE)。如果你把修改后的版本作为公开服务运行，必须公开你的修改。

```text
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (c) 2026, BoothDex contributors
```
