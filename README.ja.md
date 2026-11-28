<div align="center">

<img src="web/src/app/icon.svg" width="104" alt="BoothDex">

# BoothDex

**Rust と Next.js で構築された、BOOTH 向け VRChat アセットのセルフホスト型検索エンジンです。**

**[デプロイガイド](docs/DEPLOY.md)** (英語) / **[API](#api)** / **[ライセンス](#ライセンス)**

[English](README.md) / [简体中文](README.zh-CN.md) / 日本語

[はじめに](#はじめに) / [主な機能](#主な機能) / [ドキュメント](#ドキュメント) / [アーキテクチャ](#アーキテクチャ) / [開発](#開発)

</div>

BoothDex は、[BOOTH](https://booth.pm) で販売されている VRChat 向け商品をインデックス化し、対応アバター、カテゴリ、タグ、機能、価格などの条件で高速に絞り込める検索エンジンです。2026-11-01 に閉鎖された VRCatalogue の後継を目指して開発が始まり、互換性のある検索構文を備えています。本プロジェクトは独立したオープンソースプロジェクトであり、pixiv、BOOTH、および VRCatalogue とは一切関係ありません。商品データおよび画像の権利は各制作者に帰属し、すべての商品は BOOTH の販売ページへ直接リンクしています。

![ホーム画面（カテゴリ：髪型）: 絞り込みのサイドバー、アバター本体のカルーセル、商品グリッド](docs/screenshots/ja/home.webp)

## はじめに

BoothDex は、PostgreSQL 19 (`pgvector`)、3 つの Rust サービス、Next.js サイトを Docker または Podman Compose で起動します。ビルド済みイメージは `linux/amd64` と `linux/arm64` に対応しています。サーバーには **4 GB 以上のメモリ**と 2 GB のスワップが必要です (メモリは 6 GB 推奨)。TLS リバースプロキシ向けに公開するのは単一ポート (`WEB_LISTEN`、既定値 `127.0.0.1:3100`) だけです。

公開済みのデータバンドルを使ってインストールします。バンドルは 2026-11-21 時点のスナップショットで、約 22.3 万件の商品と販売終了商品のアーカイブ画像を収録しています (約 2.2 GB)。[デプロイガイド](docs/DEPLOY.md#optional-download-the-data-bundle) (英語) の手順でダウンロードしてから、次を実行します。

```sh
mkdir -p /srv/boothdex && cd /srv/boothdex
curl -fLO https://raw.githubusercontent.com/JohnsonRan/BoothDex/master/deploy/compose.yaml
curl -fLo .env https://raw.githubusercontent.com/JohnsonRan/BoothDex/master/deploy/.env.example
$EDITOR .env    # POSTGRES_PASSWORD、SITE_URL、WEB_LISTEN、CONTACT_EMAIL を設定 (バンドル利用時は ARCHIVE_IMAGES_DIR も指定)
docker compose pull && docker compose up -d postgres
until docker compose exec -T postgres pg_isready -h 127.0.0.1 -U catalogue -d catalogue; do sleep 2; done
docker compose exec -T postgres pg_restore -U catalogue -d catalogue --no-owner --role=catalogue \
  < /srv/boothdex/bundle/catalogue.dump
docker compose up -d
```

最後の行でスタックを起動するのは、`pg_restore` がエラーなく終わってからにします。`api` は見つけたデータベースをそのままマイグレーションするためです。バンドルを使わない場合は `pg_restore` のコマンドを省き、BOOTH をクロールします。2～3 日かかります ([データバンドルを使わない起動手順](docs/DEPLOY.md#starting-without-a-data-bundle))。新規データベースには、カテゴリツリー、スマートタグ、約 600 体のアバター本体があらかじめ登録されています。

その後、プロキシ経由でサイトを開いてアカウントを登録し、そのユーザー名を `.env` の `ADMIN_USERNAMES` に追加して `docker compose up -d api` を実行します。管理画面は `/admin` です。

## 主な機能

検索 ([詳細](docs/SEARCH.md)、英語):

- 対応アバター、スマートカテゴリ、BOOTH カテゴリ、タグ (含む・除外)、機能 (MA・lilToon・Poiyomi・VRCFury)、価格、公開日、無料のみ、年齢制限で、約 20 万件の商品を数ミリ秒のうちに絞り込めます。
- 最近の検索、アバター、商品、ショップ、タグを候補に出す検索欄と、1 語ごとに削除できるトークン。
- ブラウザーのタブには、現在の検索やカタログビューのタイトルを画面の表示言語で表示します。
- BOOTH の商品リンクまたは商品番号で、その商品を先頭に表示します。
- 日本語と中国語の全文検索。全角と半角、ひらがなとカタカナは区別せず、英字の単語は 1 文字の誤りまで許します。
- BOOTH タグは英語・日本語・韓国語・中国語の訳語でも一致するため、別の言語でタグ付けされた商品も見つかります。
- オプションで、言語をまたぐセマンティック検索と、最大 4 件の関連検索を利用できます。

閲覧とアカウント:

- カルーセルでアバター本体を選ぶと、対応する商品を一覧できます。「アバター本体」ビューではアバター本体を一覧し、性別で絞り込めます。
- 1 画面分スクロールすると「トップへ戻る」ボタンが現れ、絞り込みと結果はそのままです。
- 商品はダイアログで開き、BOOTH の全画像ギャラリー、バリエーション、タイトルの翻訳、画像ダウンロードを備えています。
- お気に入り、最近見た商品、よく使うアバターの記憶、名前を変更して RSS で購読できる保存した検索。
- 表示名とパスワードは設定で変更できます。パスワードを変更すると、ほかの端末はログアウトされます。
- 上部バーから、アカウントなしで書き込めるゲストブック (ニックネーム、500 文字・10 行まで、Cloudflare Turnstile の確認)。管理者はメッセージを非表示にしたり削除したりでき、完了または対応しないとして閉じることもできます。
- 管理者は、サインインと新規登録に Turnstile の確認を求めたり、確認していない訪問者により厳しい制限をかけたりできます ([Turnstile](docs/SERVICES.md#cloudflare-turnstile)、英語)。

<details>
<summary>「マヌカ」と入力したときの候補: アバター本体、続いて一致する商品が表示されます</summary>

![検索欄の下にアバターと商品ごとに表示される検索候補](docs/screenshots/ja/search.webp)

</details>

<details>
<summary>商品の詳細は検索結果の上に開きます</summary>

![ギャラリー、翻訳タイトル、BOOTH で購入ボタンを備えた商品ダイアログ](docs/screenshots/ja/item.webp)

</details>

外観と言語:

- インターフェイスは Meta の [Astryx](https://github.com/meta/astryx) デザインシステムで構築されています。
- ライト、ダーク、システムのモード、ダークモードの純黒、動きの軽減を選べます。動きの軽減がオフのときはシステムの設定に従います。設定はアカウントに保存されます。
- 日本語・英語・簡体字中国語・韓国語に対応し、商品タイトルの原文/翻訳切り替え、通貨換算、スマートフォン向けのレイアウトを備えています。

<details>
<summary>設定でカラーモードを選べます</summary>

![設定の外観ページ: テーマのモード、純黒モード、上部で絞り込み](docs/screenshots/ja/settings.webp)

</details>

<details>
<summary>スマートフォンでのホーム画面 (英語表示)</summary>

<p align="center"><img src="docs/screenshots/phone.webp" width="390" alt="スマートフォンでのホーム画面。フィルターは +8 ボタンにまとめられている"></p>

</details>

クローラー・AI・画像 ([詳細](docs/WORKER.md)、英語):

- `worker` が 1 時間ごとに BOOTH の新着商品を取得し、毎日全件を巡回して販売終了の判定を行い、人気商品も定期的に再確認します。
- オプションの AI エンリッチメント (OpenAI 互換 API) が、スマートカテゴリ、年齢制限、対応機能、対応アバターの紐付け、タイトル翻訳を付与し、カテゴリ名とタグ名を翻訳します。セマンティック検索用の埋め込みベクトルも同じプロバイダーで生成します。
- 画像プロキシが BOOTH の画像とショップアイコンを AVIF に再エンコードしてキャッシュします。販売終了した商品の画像はアーカイブ済みのコピーから配信します。

管理画面:

- `/admin` の管理画面では、稼働状況のリアルタイム表示と過去 30 日間の推移グラフ、ジョブの実行・停止・スケジュール、リアルタイムログと履歴を確認できます。履歴には、過去 30 日間の失敗のうち、その後の同種の実行で解消されなかったものも表示されます。
- AI エンリッチメントが諦めた商品を一覧し、1 件ずつまたはまとめて再試行できます。
- AI プロバイダーの設定と、新規登録の受付・停止を行えます。
- ユーザーを検索し、ログイン中の端末の確認とログアウト、アカウントの無効化、パスワードのリセットを行えます。
- 日本語・英語・簡体字中国語に対応しています。

<details>
<summary>管理画面の概要ページ</summary>

![管理画面の概要: ワーカーの状態、インデックス、アクティビティのカードと推移グラフ](docs/screenshots/ja/admin.webp)

</details>

商品ごとの単独ページ、価格推移グラフ、ソーシャルログイン、およびパスワード再設定メールは意図的に設けていません (パスワードは管理者がリセットします)。商品説明文やバリエーション名は翻訳せず、タイトルと BOOTH タグのみを翻訳します。

## ドキュメント

| ページ | 読者の目的 |
|---|---|
| [デプロイガイド](docs/DEPLOY.md) | サーバーの構築、設定、更新、バックアップ |
| [検索の動作](docs/SEARCH.md) | 検索語に対して検索が何をするかを知る |
| [API リファレンス](docs/API.md) | HTTP API を呼び出す |
| [バックグラウンドジョブと AI エンリッチメント](docs/WORKER.md) | worker が何を実行し、AI プロバイダーが何をするかを知る |
| [サービスの動作](docs/SERVICES.md) | 画像プロキシ、レート制限、キャッシュ、Turnstile の動作を知る |
| [計測値](docs/MEASUREMENTS.md) | ドキュメントに載せた数値を調べる |

上記のページは英語のみです。

## アーキテクチャ

### サービス構成

```mermaid
flowchart LR
  browser["ブラウザー / RSS リーダー"] --> web["web (Next.js :3100)<br/>UI と /admin"]

  subgraph stack["BoothDex サービス群"]
    web -- "/api/*" --> api["api (Rust, Axum :3000)<br/>Roaring ビットマップ・Tantivy"]
    web -- "/img/*" --> imgproxy["imgproxy (Rust :3001)<br/>foyer AVIF キャッシュ"]
    worker["worker (Rust)<br/>booth・ai・embed の 3 レーン"]
    pg[("PostgreSQL 19<br/>+ pgvector")]

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
    q["検索クエリ + 絞り込み条件"] --> parse["商品リンク・商品番号、アバター名・カテゴリ語句の抽出"]
    parse --> kw["Lindera (IPADIC, CC-CEDICT) + Tantivy<br/>+ 検索語を訳したタグ<br/>& Roaring ビットマップ絞り込み"]
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
| `crates/limit` | リバースプロキシ越しのクライアント IP と IP ごとのトークンバケット。`api` と `imgproxy` が共用します |
| `crates/api` | 検索 API。絞り込みにはメモリ上の Roaring ビットマップを使用し、Tantivy 全文検索インデックスはディスク (`INDEX_DIR`) に保持して再起動時も再利用します。ページ表示用の商品データは PostgreSQL から取得します |
| `crates/worker` | 3 レーン構成のジョブランナー: `incremental` (新着取得)、`sweep` (全件巡回)、`details`、`refresh`、`enrich`、`localize`、`embed` (AI)。`worker run` でデーモン起動、`worker once <kind> [params-json]` で単発実行します |
| `crates/imgproxy` | `/b/{size}/...` (商品画像)、`/u/{size}/...` (ショップアイコン)、`/a/{file}` (BOOTH の静的画像)、`/f/{size}/{uuid}/{file}` (pixiv Factory の画像。imgproxy がサイズに合わせて縮小) を AVIF に変換・キャッシュ配信し、`/v/{sha1}.webp` でアーカイブ画像を配信します |
| `web` | Next.js フロントエンドおよび `/admin` 管理画面 |
| `deploy` | Dockerfile、`docker-bake.hcl`、`compose.yaml`、および `.env.example` |
| `scripts` | データバンドル作成ツール、モック AI サーバー |

計測した応答時間とメモリ使用量は[計測値](docs/MEASUREMENTS.md) (英語) にあります。

## 開発

必要な環境: Rust (stable)、[Bun](https://bun.sh)、および `pgvector` 拡張入りの PostgreSQL 19 (Podman または Docker 上で実行)。

```sh
podman run -d --name boothdex-pg -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=catalogue \
  -p 5432:5432 -v boothdex-pgdata:/var/lib/postgresql docker.io/pgvector/pgvector:pg19

cargo build --release            # 初回ビルド時に IPADIC と CC-CEDICT の辞書をダウンロードします (失敗した場合は再実行してください)
# コンテナー イメージでは libaom で AVIF をエンコードします: --features imgproxy/libavif (libavif-dev と pkg-config が必要)。
# 指定しない場合は rav1e を使用します。nasm がある場合は --features imgproxy/asm を追加すると、エンコードが約 3 倍速くなります。
(cd web && bun install)

export DATABASE_URL=postgres://postgres:dev@127.0.0.1:5432/catalogue COOKIE_SECURE=0
target/release/api &             # :3000
target/release/imgproxy &        # :3001
target/release/worker run &
(cd web && bun run dev)          # :3100 (/api と /img を上記サービスへ転送)
```

`http://localhost:3100` でアカウント `admin` を新規登録し、`api` を停止してから、そのユーザー名を指定して再起動してください。API は `ADMIN_USERNAMES` に記載されたユーザー名の登録を拒否するため、アカウントを作成してから記載してください。

```sh
ADMIN_USERNAMES=admin target/release/api &   # :3000 (先に起動した api を停止してから)
```

`http://localhost:3100/admin` を開いてから、数ページ分クロールして商品データを取得してください。

```sh
target/release/worker once incremental '{"max_pages": 3, "full": true}'
```

- **モック AI プロバイダー**: API キーがない場合でも、`bun scripts/mock-ai.ts` を起動して管理画面の AI 接続先に `http://127.0.0.1:3555/v1` を指定し、任意の API キーと一緒に保存すると AI ジョブの動作を確認できます。

テストと静的解析:

```sh
cargo test --workspace -- --include-ignored && CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets -- -D warnings
python3 scripts/check-requirements.py && python3 scripts/test_check_requirements.py
cd web && bun test && bunx next typegen && bunx tsc --noEmit && bunx biome check
```

## API

`/api/search` は VRCatalogue 互換のクエリパラメーターを受け付けます: `v q sort dir page pageSize adult avatars authors tags extags btags exbtags sc scp bc if free pmin pmax range from to asup gender fields`。その他のエンドポイントとして `/api/avatars`、`/api/authors`、`/api/tags`、`/api/booth-tags` (`?q=&offset=&limit=`)、`/api/categories`、`/api/items?ids=`、`/api/palette?q=`、`/api/related?<検索クエリ>`、および `/api/rss?<検索クエリ>` があります。VRCatalogue との違い、追加のパラメーター、レスポンスのフィールドは [API リファレンス](docs/API.md) (英語) にあります。

## 謝辞

- VRCatalogue: オリジナルのサイト、検索モデル、および本プロジェクトの出発点となった分類体系
- [BOOTH](https://booth.pm): 商品データおよび画像配信元
- [Tantivy](https://github.com/quickwit-oss/tantivy) および [Lindera](https://github.com/lindera/lindera): 全文検索と日本語・中国語の形態素解析
- [CC-CEDICT](https://www.mdbg.net/chinese/dictionary?page=cedict) ([CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)): 中国語タイトルの分かち書きに使う、`api` に組み込まれた中国語辞書 (Lindera の CC-CEDICT-MeCab 経由)
- IPADIC (奈良先端科学技術大学院大学): `api` に組み込まれた日本語辞書。両辞書のライセンス文は [deploy/licenses](deploy/licenses) にあり、`api` イメージの `/usr/share/doc/boothdex-api/` にも含まれます
- [roaring-rs](https://github.com/RoaringBitmap/roaring-rs): 絞り込み用ビットマップ
- [foyer](https://github.com/foyer-rs/foyer): ハイブリッド画像キャッシュ
- [Astryx](https://github.com/meta/astryx): デザインシステム、UI コンポーネント、スタイル

## ライセンス

[AGPL-3.0-or-later](LICENSE) の下で提供されています。改変したバージョンを公開サービスとして運用する場合は、変更後のソースコードを公開する必要があります。

```text
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (c) 2026, BoothDex contributors
```
