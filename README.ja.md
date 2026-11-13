<div align="center">

<img src="web/src/app/icon.svg" width="104" alt="BoothDex">

# BoothDex

**Rust と Next.js で構築された、BOOTH 向け VRChat アセットのセルフホスト型検索エンジンです。**

**[デプロイガイド](docs/DEPLOY.md)** (英語) / **[API](#api)** / **[ライセンス](#ライセンス)**

[English](README.md) / [简体中文](README.zh-CN.md) / 日本語

[はじめに](#はじめに) / [主な機能](#主な機能) / [アーキテクチャ](#アーキテクチャ) / [開発](#開発)

</div>

BoothDex は、[BOOTH](https://booth.pm) で販売されている VRChat 向け商品をインデックス化し、対応アバター、カテゴリ、タグ、機能、価格などの条件で高速に絞り込める検索エンジンです。2026-11-01 に閉鎖された VRCatalogue の後継を目指して開発が始まり、互換性のある検索構文を備えています。本プロジェクトは独立したオープンソースプロジェクトであり、pixiv、BOOTH、および VRCatalogue とは一切関係ありません。商品データおよび画像の権利は各制作者に帰属し、すべての商品は BOOTH の販売ページへ直接リンクしています。

![ホーム画面（カテゴリ：髪型）: 絞り込みのサイドバー、アバター本体のカルーセル、商品グリッド](docs/screenshots/ja/home.webp)

## はじめに

詳細な構築手順、リバースプロキシの設定例、バックアップ運用については [デプロイガイド](docs/DEPLOY.md) (英語) を参照してください。Docker または Podman Compose を使用して PostgreSQL 17 (`pgvector`)、3 つの Rust サービス、Next.js サイトを起動し、TLS リバースプロキシ向けに単一ポート (`WEB_LISTEN`、既定値 `3100`) のみを公開します。**6 GB 以上のメモリ**を備えた Linux サーバーが必要です (8 GB 推奨、スワップ併用時は 4 GB から動作可能)。

```mermaid
flowchart LR
  env["1. compose.yaml と .env を配置<br/>(パスワード・SITE_URL を設定)"] --> data{"2. 初期データの準備"}

  data -- "データバンドル (約 5 分)" --> bundle["images.tar を展開<br/>pg_restore で catalogue.dump を復元"]
  data -- "空の DB から取得 (約 2～3 日)" --> crawl["カテゴリと約 380 体のアバターを自動投入<br/>初回フルクロールを実行"]

  bundle --> up["3. docker compose up -d"]
  crawl --> up
  up --> admin["4. アカウント登録 → ADMIN_USERNAMES 設定<br/>/admin で AI プロバイダーを設定 (任意)"]
```

新規データベースには、起動時のマイグレーションによってカテゴリツリー、スマートタグ、および約 380 体のアバター本体が自動的に登録されます。数日かかる初回フルクロール ([データバンドルを使わない起動手順](docs/DEPLOY.md#starting-without-a-data-bundle)) を省略したい場合は、[公開済みデータバンドル](https://github.com/JohnsonRan/BoothDex/releases/tag/data-2026-11-04) (2026-11-04 時点のスナップショット、約 21.7 万件の商品と販売終了商品のアーカイブ画像を収録、約 2.1 GB) を復元してください。

```sh
mkdir -p /srv/boothdex && cd /srv/boothdex
curl -fLO https://raw.githubusercontent.com/JohnsonRan/BoothDex/master/deploy/compose.yaml
curl -fLo .env https://raw.githubusercontent.com/JohnsonRan/BoothDex/master/deploy/.env.example
$EDITOR .env    # POSTGRES_PASSWORD、SITE_URL、WEB_LISTEN、CONTACT_EMAIL を設定 (バンドル利用時は ARCHIVE_IMAGES_DIR も指定)
docker compose pull && docker compose up -d
```

## 主な機能

### 検索

- 約 20 万件の商品を、対応アバター、スマートカテゴリ、BOOTH カテゴリ、タグ (含む・除外)、機能 (MA・lilToon・Poiyomi・VRCFury)、価格、公開日、無料のみ、年齢制限で数ミリ秒のうちに絞り込めます。
- フィルター行には検索欄とフィルターごとのセレクターが並び、入りきらないフィルターは「+N」ボタンにまとめられます。
- 検索欄は入力に合わせて最近の検索、アバター、商品、ショップ、タグを候補として表示します。`Ctrl+K` (macOS では `Cmd+K`) で検索欄にフォーカスできます。
- 検索語は 1 語ずつトークンとして表示され、個別に削除できます。アバターやカテゴリとして認識された語は 1 つのトークンにまとまり、先頭にアバターの画像またはカテゴリのアイコンが付きます。
- 日本語の全文検索には Lindera (IPADIC) を使用します。全角と半角、ひらがなとカタカナは区別せずに検索します (“しなの” でシナノもヒット)。オプションのセマンティック検索を有効にすると、言語をまたいで意味の近い商品 (“哥特裙” でゴスロリ衣装がヒットするなど) を検索し、キーワード検索の結果と統合して並べ替えます。

<details>
<summary>「マヌカ」と入力したときの候補: アバター本体、続いて一致する商品が表示されます</summary>

![検索欄の下にアバターと商品ごとに表示される検索候補](docs/screenshots/ja/search.webp)

</details>

### 閲覧とアカウント

- 検索結果の上にあるカルーセルでアバター本体を選ぶと、そのアバターに対応する商品を一覧できます。「アバター本体」ビューではアバター本体そのものを一覧し、性別で絞り込めます。
- 商品はダイアログで開き、BOOTH の全画像ギャラリー、バリエーション、タイトルの翻訳、画像ダウンロードを備えています。
- お気に入り、最近見た商品 (検索結果に「閲覧済み」と表示)、よく使うアバターの記憶、および保存した検索に対応しています。保存した検索は名前を変更でき、RSS で購読することもできます。
- 表示名とパスワードは設定で変更できます。パスワードを変更すると、ほかの端末はログアウトされます。

<details>
<summary>商品の詳細は検索結果の上に開きます</summary>

![ギャラリー、翻訳タイトル、BOOTH で購入ボタンを備えた商品ダイアログ](docs/screenshots/ja/item.webp)

</details>

### 外観と言語

- インターフェイスは Meta の [Astryx](https://github.com/meta/astryx) デザインシステムで構築されています。
- ライト、ダーク、システムのいずれかのモードを選べます。ダークモードでは純黒モードも選べます。サインイン中は選択内容がアカウントに保存されます。
- 表示言語は日本語・英語・簡体字中国語・韓国語に対応しています。商品タイトルの原文/翻訳切り替え、通貨換算表示に対応し、スマートフォンでも利用できます。

<details>
<summary>設定でカラーモードを選べます</summary>

![設定の外観ページ: テーマのモード、純黒モード、上部で絞り込み](docs/screenshots/ja/settings.webp)

</details>

<details>
<summary>スマートフォンでのホーム画面 (英語表示)</summary>

<p align="center"><img src="docs/screenshots/phone.webp" width="390" alt="スマートフォンでのホーム画面。フィルターは +8 ボタンにまとめられている"></p>

</details>

### クローラー・AI・画像

- `worker` が 1 時間ごとに BOOTH の新着商品を取得し、毎日全件を巡回して価格更新や販売終了の判定を行い、人気商品も定期的に再確認します。
- AI エンリッチメント *(オプション、任意の OpenAI 互換 API に対応、推論強度を設定可能)*: 新着商品に対してスマートカテゴリ、年齢制限、対応機能、および 4 言語 (日本語・英語・簡体字中国語・韓国語) のタイトル翻訳を自動付与し、名前照合による対応アバターの紐付けやカテゴリ・タグ名の翻訳も行います。セマンティック検索用の埋め込みベクトルも同じプロバイダーで生成します。
- 画像プロキシが BOOTH の画像とショップアイコンを AVIF に再エンコードしてキャッシュします。販売終了した商品の画像はアーカイブ済みのコピーから配信します。

### 管理画面

- `/admin` の管理画面では、概要ページで稼働状況を SSE でリアルタイムに表示し、過去 30 日間の推移グラフを確認できます。
- ジョブの実行と停止、各ジョブのリアルタイムログの確認、スケジュールのオン/オフ切り替え、ジョブ履歴の閲覧ができます。
- AI プロバイダーの設定と、新規登録の受付・停止を行えます。
- ユーザーを検索し、ログイン中の端末の確認とログアウト、アカウントの無効化、パスワードのリセットを行えます。パスワードのリセットまたはアカウントの再有効化で、ログインのロックも解除されます。
- 管理画面は日本語・英語・簡体字中国語に対応しています。

<details>
<summary>管理画面の概要ページ</summary>

![管理画面の概要: ワーカーの状態、インデックス、アクティビティのカードと推移グラフ](docs/screenshots/ja/admin.webp)

</details>

商品ごとの単独ページ、価格推移グラフ、ソーシャルログイン、およびパスワード再設定メールは意図的に設けていません (パスワードは管理者がリセットします)。商品説明文やバリエーション名は翻訳せず、言語横断検索に必要なタイトルのみを翻訳します。

## アーキテクチャ

### サービス構成

```mermaid
flowchart LR
  browser["ブラウザー / RSS リーダー"] --> web["web (Next.js :3100)<br/>UI と /admin"]

  subgraph stack["BoothDex サービス群"]
    web -- "/api/*" --> api["api (Rust, Axum :3000)<br/>Roaring ビットマップ・Tantivy"]
    web -- "/img/*" --> imgproxy["imgproxy (Rust :3001)<br/>foyer AVIF キャッシュ"]
    worker["worker (Rust)<br/>booth・ai・embed の 3 レーン"]
    pg[("PostgreSQL 17<br/>+ pgvector")]

    api --> pg
    worker --> pg
    api --- idx[("INDEX_DIR<br/>(Tantivy インデックス)")]
    imgproxy --- cache[("imgcache と<br/>アーカイブ WebP")]
  end

  imgproxy --> pximg["booth.pximg.net"]
  imgproxy --> asset["asset.booth.pm"]
  imgproxy --> factory["factory.pixiv.net"]
  worker --> booth["booth.pm"]
  worker -. "分類・翻訳・埋め込み" .-> ai["OpenAI 互換 API"]
  api -. "検索クエリ埋め込み" .-> ai
```

### ハイブリッド検索とデータ更新フロー

```mermaid
flowchart TD
  subgraph query["ハイブリッド検索 (/api/search)"]
    q["検索クエリ + 絞り込み条件"] --> parse["アバター名・カテゴリ語句の抽出"]
    parse --> kw["Lindera (IPADIC) + Tantivy<br/>& Roaring ビットマップ絞り込み"]
    parse -. "セマンティック検索有効時" .-> embq["検索ベクトル取得<br/>(メモリ → DB キャッシュ → AI API、上限 2.5 秒)"]
    embq -.-> ann["pgvector HNSW 近傍検索"]
    kw --> merge["スコア統合・ファセット集計"]
    ann -.-> merge
    merge --> hydrate["PostgreSQL から表示ページの商品を取得"]
  end

  subgraph ingest["バックグラウンドジョブと無停止リロード"]
    crawl_job["booth レーン:<br/>incremental・sweep・refresh"] --> dbw[("PostgreSQL")]
    ai_job["ai レーン:<br/>enrich (200 件ずつ連続実行)・localize"] --> dbw
    ai_job -. "連続実行の完了後" .-> emb_job["embed レーン:<br/>1024 次元 halfvec 生成"]
    emb_job --> dbw
    dbw -- "catalog_meta.version を更新" --> snap["api が裏で新スナップショットを構築<br/>ArcSwap でアトミックに切り替え"]
  end
```

### ディレクトリ構成

| パス | 内容 |
|---|---|
| `crates/core` | 共通型、BOOTH クライアント、および起動時に自動適用される `migrations/` 内の PostgreSQL スキーマ |
| `crates/api` | 検索 API。絞り込みにはメモリ上の Roaring ビットマップを使用し、Tantivy 全文検索インデックスはディスク (`INDEX_DIR`) に保持して再起動時も再利用します。ページ表示用の商品データは PostgreSQL から取得します |
| `crates/worker` | 3 レーン構成のジョブランナー: `incremental` (新着取得)、`sweep` (全件巡回)、`details`、`refresh`、`enrich`、`localize`、`embed` (AI)。`worker run` でデーモン起動、`worker once <kind> [params-json]` で単発実行します |
| `crates/imgproxy` | `/b/{size}/...` (商品画像)、`/u/{size}/...` (ショップアイコン)、`/a/{file}` (BOOTH の静的画像)、`/f/{size}/{uuid}/{file}` (pixiv Factory の画像。imgproxy がサイズに合わせて縮小) を AVIF に変換・キャッシュ配信し、`/v/{sha1}.webp` でアーカイブ画像を配信します |
| `web` | Next.js フロントエンドおよび `/admin` 管理画面 |
| `deploy` | Dockerfile、`docker-bake.hcl`、`compose.yaml`、および `.env.example` |
| `scripts` | データバンドル作成ツール、モック AI サーバー |

32 スレッド環境で約 21.6 万件の商品を検索した場合、API 内部の処理時間は 1～6 ミリ秒、リクエスト全体では 5～40 ミリ秒で応答し、32 クライアント同時接続時に毎秒約 550 リクエストを処理します。Linux コンテナー上での `api` のメモリ使用量は通常時で約 0.4～1 GB (メモリマップされた Tantivy インデックスはページ キャッシュ側)、カタログ再読み込み時のピークで約 1.8 GB です (`mem_limit: 4g`)。

## 開発

必要な環境: Rust (stable)、[Bun](https://bun.sh)、および `pgvector` 拡張入りの PostgreSQL 17 (Podman または Docker 上で実行)。

```sh
podman run -d --name boothdex-pg -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=catalogue \
  -p 5432:5432 -v boothdex-pgdata:/var/lib/postgresql/data docker.io/pgvector/pgvector:pg17

cargo build --release            # 初回ビルド時に IPADIC 辞書をダウンロードします (失敗した場合は再実行してください)
# コンテナー イメージでは libaom で AVIF をエンコードします: --features imgproxy/libavif (libavif-dev と pkg-config が必要)。
# 指定しない場合は rav1e を使用します。nasm がある場合は --features imgproxy/asm を追加すると、エンコードが約 3 倍速くなります。
(cd web && bun install)

export DATABASE_URL=postgres://postgres:dev@127.0.0.1:5432/catalogue COOKIE_SECURE=0 ADMIN_USERNAMES=admin
target/release/api &             # :3000
target/release/imgproxy &        # :3001
target/release/worker run &
(cd web && bun run dev)          # :3100 (/api と /img を上記サービスへ転送)
```

アカウント `admin` を新規登録し、`http://localhost:3100/admin` を開いてから、数ページ分クロールして商品データを取得してください。

```sh
target/release/worker once incremental '{"max_pages": 3, "full": true}'
```

- **モック AI プロバイダー**: API キーがない場合でも、`bun scripts/mock-ai.ts` を起動して管理画面の AI 接続先に `http://127.0.0.1:3555/v1` を指定すると AI ジョブの動作を確認できます。

テストと静的解析:

```sh
cargo test --workspace -- --include-ignored && cargo clippy --workspace --all-targets -- -D warnings
python3 scripts/check-requirements.py && python3 scripts/test_check_requirements.py
cd web && bun test && bunx next typegen && bunx tsc --noEmit && bunx biome check
```

## API

`/api/search` は VRCatalogue 互換のクエリパラメーターを受け付けます: `v q sort dir page pageSize adult avatars authors tags extags btags exbtags sc scp bc if free pmin pmax range from to asup gender fields`。その他のエンドポイントとして `/api/avatars`、`/api/authors`、`/api/tags`、`/api/booth-tags` (`?q=&offset=&limit=`)、`/api/categories`、`/api/items?ids=`、`/api/palette?q=`、および `/api/rss?<検索クエリ>` があります。

VRCatalogue との違い: 機能 (Features) のファセット件数が選択中のカテゴリを反映します。「人気」(Trending) 順は経過時間で減衰させた「スキ!」数で算出します。`range` パラメーターでは `7d`、`30d`、`90d` も指定できます。`fields=card` でカード表示用の軽量データを取得できます (bio、variations、booth_tags、linked_avatars、categories を空にし、translations はタイトルのみ保持、booth_images/media は最大 2 枚に制限。デフォルト応答は完全なまま。完全な記録は `/api/items?ids=` で取得)。

## 謝辞

- VRCatalogue: オリジナルのサイト、検索モデル、および本プロジェクトの出発点となった分類体系
- [BOOTH](https://booth.pm): 商品データおよび画像配信元
- [Tantivy](https://github.com/quickwit-oss/tantivy) および [Lindera](https://github.com/lindera/lindera): 全文検索と日本語形態素解析
- [roaring-rs](https://github.com/RoaringBitmap/roaring-rs): 絞り込み用ビットマップ
- [foyer](https://github.com/foyer-rs/foyer): ハイブリッド画像キャッシュ
- [Astryx](https://github.com/meta/astryx): デザインシステム、UI コンポーネント、スタイル

## ライセンス

[AGPL-3.0-or-later](LICENSE) の下で提供されています。改変したバージョンを公開サービスとして運用する場合は、変更後のソースコードを公開する必要があります。

```text
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (c) 2026, BoothDex contributors
```
