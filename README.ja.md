# OxidePress

[RustPress](https://github.com/LegacyToRustProject/RustPress)（Ryuji Yasukochi）の独立したフォーク。上流が止まったため、このツリーは同じ GPL-2.0-only の作業を新しい名前で続ける。元の `LICENSE` は変えていない。[AUTHORS](AUTHORS) を参照。

[![CI](https://github.com/Vabian124/OxidePress/actions/workflows/ci.yml/badge.svg)](https://github.com/Vabian124/OxidePress/actions/workflows/ci.yml)
[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)
[![Rust: 1.97+](https://img.shields.io/badge/Rust-1.97%2B-orange.svg)](https://www.rust-lang.org/)

[English](README.md)

OxidePress は、WordPress のデータベースを Rust のバイナリから配信する。このツリーのテーマは Tera テーマである。PHP テーマは動かない。置いた PHP プラグインは、1 つのワーカーで起動し、フックと head/footer の echo を足すことができる。それは部分的な動作である。WordPress ではない。保存された SEO、フィールド、フォーム、セキュリティ、ショップの行を扱う経路は、ネイティブの画面のままである。すべてのプラグインを AI で変換することは、今のランタイムではない。[`oxidepress-rustify`](crates/oxidepress-rustify) は、後から使う骨格として残してよい。

WordPress 7.1.4 に対するスキーマと動作の記録は [docs/compatibility-plan.md](docs/compatibility-plan.md)。残りの作業は [docs/TODO.md](docs/TODO.md) で、同じ一覧が [docs/progress.json](docs/progress.json) にある。2026-11-07 のローカルサイトの状態は [docs/alpha-status.md](docs/alpha-status.md)。

## プラグインの二つの経路

1. ネイティブの Rust 画面は、Yoast、ACF、Contact Form 7、Wordfence、WooCommerce と同じ種類の行を読み書きする。クレートは `oxidepress-seo`、`oxidepress-fields`、`oxidepress-forms`、`oxidepress-security`、`oxidepress-commerce`。WordPress.org のプラグインを完全に再実装した、という主張ではない。
2. ドロップインの PHP。[`oxidepress-php`](crates/oxidepress-php) がランタイムである。`oxidepress-rustify` はパーサの骨格であり、ランタイムではない。

PHP ランタイムは、長く生きるシステム `php` CLI ワーカーが一つである。Rust とは長さ付き JSON で話す。フックとショートコードは `oxidepress-core` に付く。フックごとの PHP-FPM ではない。リクエストごとの FPM 呼び出しでもない。WordPress プロセスの前に置いた Rust でもない。[ADR-001](docs/adr/001-php-bridge-mode.md) はそれらを却下した。[ADR-002](docs/adr/002-php-worker.md) は、ドロップイン経路について、この一つのワーカーを採用する。

## ドロップインのプラグインにできること

検出は、トップレベルの `*.php`、または一つ下のディレクトリにある PHP ファイルの先頭 8 KiB にある `Plugin Name:` ヘッダを読む。それより深くは再帰しない。プラグインは WordPress の basename（`hello-dolly/hello.php`）で登録される。有効化は表示名でも受け付ける。

起動時、有効なプラグインに `PluginType::Php` が一つでもあれば、ワーカーを一つだけ起動する。プログラムは `OXIDEPRESS_PHP`、または `PATH` 上の `php` である。`php` が無いときもサイトは起動し、それらのプラグインは Error になる。

`wp_head` と `wp_footer` の echo は、テーマの HTML に追記される。

2026-11-07 のプローブ（PHP 8.3.36）は [crates/oxidepress-php/REAL-PLUGINS.md](crates/oxidepress-php/REAL-PLUGINS.md)。Akismet、Classic Editor、Classic Widgets、Disable Comments、Yoast Duplicate Post、Hello Dolly、Health Check はプローブのフックを終えた。Contact Form 7 は、WordPress のファイルシステム API を読み込むところで止まった。このツリーは `wp-admin` も `wp-includes` も同梱せず、その require をスタブもしない。

## ドロップインのプラグインにできないこと

- プラグインが `wp-content/plugins` にあるとき、`ABSPATH` は `wp-content` を含むディレクトリである。そうでなければ、`wp-load.php` がある祖先を使う。どちらも無いときは、プラグインディレクトリの親である。プラグインディレクトリそのものではなく、このツリーは `wp-admin` も `wp-includes` も同梱しない。
- プラグインを一つ無効化すると、そのフックとショートコードは止まり、他のプラグインは動き続ける。再度有効化するときは、ワーカーに読み込み済みのコピーを使う。
- プラグインを有効化すると、スラッシュを含む WordPress の basename を保存する。basename が無いプラグインは表示名を保存する。復元はどちらの形も受け付ける。
- `wp_enqueue_style` は `wp_head` に出力する。`wp_enqueue_script` は `wp_head` に出力し、フッター指定のスクリプトは `wp_footer` に出力する。管理者は `/wp-admin/admin.php?page={slug}` でプラグインの設定画面を開く。その画面が登録したオプションは `/wp-admin/options.php` に保存する。`siteurl`、`active_plugins`、権限オプション、ソルト、`rustpress_*` キーは拒否する。
- `init` で投稿タイプを登録したプラグインは、その行を保存し、読み、更新し、削除できる。管理者は `/wp-admin/edit.php?post_type={name}` で一覧と編集を行う。公開アドレスは `/{slug}/` である。リビジョンとテンプレートのタイプは拒否する。`show_ui` のタイプはブロックエディタで開き、保存時に拒否されたタイプへは切り替えられない。
- `is_admin()` は `/wp-admin` と `/wp-login.php` で true。`current_user_can()` はログイン中の権限に従い、訪問者は false。プラグイン設定画面の描画中は `manage_options` が true のまま。`is_multisite()` は false。`register_activation_hook` は、管理者がそのプラグインを有効化したときに一度だけ走る。起動のたびに再実行はしない。
- `wp_remote_get` と `wp_remote_post` は、公開された HTTP と HTTPS に届く。プライベートアドレス、内部ホスト名、それ以外のスキームは拒否する。リダイレクトは追わない。
- `ABSPATH . 'wp-includes/version.php'` が無いとき、Health Check はいまも警告する。`ABSPATH . 'wp-admin/includes/file.php'` が無いとき、Contact Form 7 はいまも止まる。

マルチサイト、WooCommerce のルックアップテーブル、バリエーション、クーポン、配送、税、決済ゲートウェイ、Elementor、Jetpack は、今の計画の外である。

## 起動

Rust 1.97 以上、MySQL 8 または MariaDB 10.5 以上。`.env.example` を `.env` にコピーし、`DATABASE_URL` を WordPress のデータベースに向ける。`RUSTPRESS_HOST` と `RUSTPRESS_PORT` は、その名前のままである。未設定なら `127.0.0.1:3000` で待ち受ける。`SKIP_MIGRATIONS=true` は、そのデータベースに既にあるテーブルを使う。

```powershell
cargo run -p oxidepress-server
```

リリースバイナリは `oxidepress`（Windows では `oxidepress.exe`）で、`oxidepress-server` から出る。CLI は `oxidepress-cli`。

`docker-compose.yml` はエンドツーエンドのハーネスである。製品のインストール手順ではない。

## CI

`main` とプルリクエストでは、GitHub Actions が Check、Test、Format、Clippy、Build Release、Security Audit を実行する。`RUSTFLAGS=-Dwarnings`。ツールチェインは Rust 1.97。Release Build は `v*` タグのときだけ実行する。

Wasmtime は `=50.0.1` にピンしている。42 系には、後のアドバイザリを解消するパッチリリースが無い。`cargo audit` は三つのアドバイザリを無視する。理由は [`.github/workflows/ci.yml`](.github/workflows/ci.yml) にある。

- `RUSTSEC-2023-0071` — rsa の Marvin 攻撃。上流の修正は無い
- `RUSTSEC-2024-0436` — paste はメンテナンスされていない
- `RUSTSEC-2026-0235` — rkyv 0.7 は、このワークスペースが有効にしていない `rust_decimal` のオプション機能である

最後の無視は、rkyv を上げたという意味ではない。

## ドキュメント

- [アーキテクチャ](docs/ARCHITECTURE.md)
- [コントリビューション](CONTRIBUTING.md)
- [互換計画](docs/compatibility-plan.md)
- [ADR 一覧](docs/adr/README.md)

## ライセンス

GPL-2.0-only。WordPress と同じライセンス。[LICENSE](LICENSE) を参照。
