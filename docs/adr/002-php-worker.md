# ADR-002: ドロップインプラグインのための PHP CLI ワーカー一つ

- **ステータス**: 承認
- **日付**: 2026-11-07
- **上書きする範囲**: [ADR-001](001-php-bridge-mode.md) の「プラグインは AI 変換のみで、PHP ランタイムは無い」という結論。対象はドロップイン経路
- **関連**: [crates/oxidepress-php/REAL-PLUGINS.md](../../crates/oxidepress-php/REAL-PLUGINS.md)

---

## 結論（先に読む人向け）

ドロップインの PHP プラグインは、長く生きるシステム `php` CLI ワーカー一つで動く。OxidePress が既に持つ行、つまり SEO、フィールド、フォーム、セキュリティ、ショップは、ネイティブのクレートが持ち続ける。`oxidepress-rustify` はパーサの骨格であり、ランタイムではない。

ADR-001 は、フックごとの PHP-FPM、リクエストごとの FPM 呼び出し、WordPress プロセス全体の前に置く Rust を、いまも却下する。

## コンテキスト

OxidePress は、WordPress の MySQL データベースを Rust のバイナリから配信する。このツリーのテーマは Tera テーマである。PHP テーマは動かない。

ADR-001（2026-03-07）は、AI による全変換と、PHP ランタイムを持たない方針を選んだ。サイトは、いまもプラグインディレクトリに PHP ファイルを置き、そのフックが登録されることを期待できる。そのファイルが起動する前に、すべてのプラグインを作り直すことは、このツリーの経路ではない。

`oxidepress-php` に入ったワーカーは、プロセスが一つである。Rust は長さ付き JSON を送る。ワーカーは有効な PHP プラグインを include し、フックとショートコードの登録を返し、Rust がそのタグを発火したときにコールバックを実行する。`wp_head` と `wp_footer` の echo は、テーマの HTML に追記される。フックとショートコードは `oxidepress-core` が持つ。

## 検討した案

### フックごとの PHP-FPM — 却下のまま

ADR-001 の案 A。1 ページは多数のフックを発火する。フックごとに FastCGI を往復させると、得より損が大きい。ADR-002 はこれを復活させない。

### リクエストごとの PHP-FPM 一回 — 却下のまま

ADR-001 の案 B。それは WordPress のブートストラップの前に置いた Rust のプロキシである。ADR-002 はこれを復活させない。

### WordPress プロセス全体の前に置く Rust — 却下のまま

ADR-001 の案 B と案 D。WordPress の前の Nginx が、すでにこれを行う。OxidePress が CMS である。ワーカーは WordPress ツリーではない。

### すべてのプラグインの AI 変換を今のランタイムにする — この決定ではない

ADR-001 の変換方針は、後の作業として残してよい。`oxidepress-rustify` は、プラグインヘッダと、リテラルの `add_action`、`add_filter`、`add_shortcode` を読み、Rust の骨格を出す。PHP は実行しない。ドロップインのランタイムではない。

### PHP CLI ワーカー一つ — 採用

`OXIDEPRESS_PHP` または `PATH` の `php` を、有効なプラグインに `PluginType::Php` が一つでもあるときだけ、一度起動する。`php` が無いときもサイトは起動し、それらのプラグインは Error になる。検出は、トップレベルの `*.php`、または一つ下のディレクトリの PHP ファイルの先頭 8 KiB にある `Plugin Name:` を読む。

Yoast、ACF、Contact Form 7、Wordfence、WooCommerce が保存する行は、ネイティブの画面に残す。`oxidepress-seo`、`oxidepress-fields`、`oxidepress-forms`、`oxidepress-security`、`oxidepress-commerce` である。これらの画面は、WordPress.org のプラグインを完全に再実装した、という主張ではない。

## 決定

ドロップインプラグインには PHP CLI ワーカーを一つ。OxidePress が既に持つ行にはネイティブのクレート。

## 根拠

置いたファイルは、WordPress を起動せず、フックごとの FastCGI も無しに、フックを登録できる。ネイティブの画面は、これら五つのプラグインが使うオプションと投稿の行を、既に読み書きする。二つの経路を分けると、サーバーは Rust の CMS のままになり、PHP ファイルには実行する場所ができる。

2026-11-07 のプローブ（PHP 8.3.36）が動作の記録である。Akismet 5.7.3a16、Classic Editor 1.7.0、Classic Widgets 0.3、Disable Comments 2.9.1、Yoast Duplicate Post 4.7、Hello Dolly 1.7.2、Health Check 1.7.1 は、プローブのフックを終える。Contact Form 7 6.2.4.1 は、`includes/filesystem.php` が `ABSPATH . 'wp-admin/includes/file.php'` を `require_once` するところで止まる。この決定は `wp-admin/includes` を同梱せず、そのファイルをスタブもしない。

## 影響

ワーカーがすること:

- プラグインを WordPress の basename（`hello-dolly/hello.php`）で登録する。有効化は表示名でも受け付ける。
- フックとショートコードを `oxidepress-core` に付ける。
- `wp_head` と `wp_footer` の echo をテーマの HTML に追記する。

ワーカーがしないこと:

- `ABSPATH` はプラグインディレクトリにスラッシュを足した値である。本物の WordPress ルートとしては誤りである。`wp-admin` や `wp-includes` を同梱せずに本物のルートを与える作業は、後で `oxidepress-php` が行う。
- プラグインを一つ無効化するときは、プロセスの再起動が要る。フックのレジストリは、コールバックを一つだけ外せない。
- `POST /admin/plugins/{name}/activate` はスラッシュを含められない。そのため `oxidepress-admin` は、いまも表示名を保存する。復元はどちらの形も受け付ける。
- 投稿タイプと管理メニューはワーカーの中に留まる。`wp_enqueue_script`、`wp_enqueue_style`、`register_setting` は受け付けるが、アセットを出力せず、設定画面も描かない。
- `current_user_can()` は false。`is_admin()` と `is_multisite()` は false。有効化コールバックは読み込み時には走らない。
- ワーカーからの HTTP は無い。
- Health Check は `ABSPATH . 'wp-includes/version.php'` でいまも警告する。このプロセスは WordPress ツリーではない。

`.php` ファイルへのリクエストは、サーバーのミドルウェアがいまも拒否する。ワーカーは、サーバーが起動する別プロセスである。ウェブルートへ `.php` をアップロードするリクエストではない。
