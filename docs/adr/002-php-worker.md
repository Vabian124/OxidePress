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
- `wp_remote_get` と `wp_remote_post` は、公開された HTTP と HTTPS に届く。

ワーカーがしないこと:

- プラグインが `wp-content/plugins` にあるとき、`ABSPATH` は `wp-content` を含むディレクトリである。そうでなければ、`wp-load.php` がある祖先を使う。どちらも無いときは、プラグインディレクトリの親である。`wp-admin` と `wp-includes` は同梱しない。
- プラグインを一つ無効化するときは、プロセスの再起動が要る。フックのレジストリは、コールバックを一つだけ外せない。
- `oxidepress-admin` は、スラッシュを含む WordPress の basename を保存する。basename が無いプラグインは表示名を保存する。復元はどちらの形も受け付ける。
- `wp_enqueue_style` は `wp_head` に `link` タグを出力する。`wp_enqueue_script` は `wp_head` に `script` タグを出力し、フッター指定のときは `wp_footer` に出力する。ホストがまだ `wp_enqueue_scripts` を発火していないとき、ワーカーは最初の出力の前にそれを一度だけ実行する。
- 管理者は `add_menu_page`、`add_options_page`、`add_submenu_page` で登録されたプラグイン画面を `/wp-admin/admin.php?page={slug}` で開く。ワーカーは `admin_menu` を一度実行し、その画面のコールバックを出力する。`/wp-admin/options.php` に送られた `register_setting` のオプションは、そのグループのものだけ保存する。`siteurl`、`home`、`active_plugins`、`template`、`stylesheet`、`wp_user_roles`、`users_can_register`、`default_role`、ソルト、`rustpress_*` キーは、プラグインが登録しても拒否する。投稿タイプの一覧画面はない。
- `is_admin()` と `current_user_can()` はその画面の描画と保存の間だけ true。`is_multisite()` は false。有効化コールバックは読み込み時には走らない。
- プライベートアドレス、内部ホスト名、HTTP 以外のスキームは拒否する。リダイレクトは追わない。1 MiB を超える応答は拒否する。
- `ABSPATH . 'wp-includes/version.php'` が無いとき、Health Check はいまも警告する。`ABSPATH . 'wp-admin/includes/file.php'` が無いとき、Contact Form 7 はいまも止まる。

`.php` ファイルへのリクエストは、サーバーのミドルウェアがいまも拒否する。ワーカーは、サーバーが起動する別プロセスである。ウェブルートへ `.php` をアップロードするリクエストではない。
