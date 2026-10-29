# Cluebench

Cluebenchは、断片的・不確実なパスワードの手がかりを整理し、複数の解釈や仮説を比較しながら、制約と候補生成戦略へ結び付けることを目指すローカル完結型のデスクトップアプリケーションです。

CTF、本人のパスワード復旧、明示的に許可されたセキュリティ分析を主な用途とします。パスワードの検証・クラッキングエンジン自体を提供したり、オンライン認証への自動試行を行ったりすることは目的としません。

> **現在の状況：** Rust Coreに、ドメイン検証・仮説の有効化・候補評価、限定的な制約解析と、由来を保持する上限付きseedベース生成を実装しています。最初のTauri 2 Desktop Shellは、React画面と合成データによる読み取り専用のCore接続確認を提供します。手がかり編集・候補プレビューUI・wordlist出力・永続化と残りのMVPワークフローは、まだ実装・リリースされていません。環境別の検証状況は[Desktop検証報告（英語）](docs/desktop-verification.md)を参照してください。

## Cluebenchの中心的な考え方

- 手がかりの原文・出典と、それに対する解釈・推測を区別する。
- 一つの手がかりに対する複数の仮説を保存し、採用する仮説を切り替える。
- 有効な制約や矛盾を可視化し、理由を追跡できるようにする。
- 手動seedと限定的な変換から候補をプレビューし、テキスト出力する。

既存のクラッキングエンジンや辞書生成器の機能数・処理速度で競うのではなく、**何を根拠に、なぜその候補を試すのかを整理する作業**を支援します。

## 技術方針

Rust Core、React + TypeScriptフロントエンドを使用し、CoreはUIから独立させます。Desktop ShellにはTauri 2を採用し、小さなRust adapterから既存Coreを呼び出します。MVPには実行時AIを搭載しません。CLI、外部向け安定Rust API、HTTP APIはMVPの対象外です。

MVPの範囲は[製品スコープ（英語）](docs/product-scope.md)を参照してください。

実装済みの意味論・資源上限は[Rust Coreの技術契約（英語）](docs/core-contract.md)、Linux/WSL2とWindows nativeの必要環境・構成・権限・検証コマンドは[Desktop開発ガイド（英語）](docs/desktop-development.md)に記載しています。

ガイドのnative依存、`apps/desktop/.node-version`のNode.js、`apps/desktop/package.json`のpnpmを導入した後、次で起動します。

```sh
pnpm -C apps/desktop install --frozen-lockfile
pnpm -C apps/desktop tauri dev
```

Shellは合成データのみを使い、パスワード入力は受け付けません。`pnpm -C apps/desktop dev`はFrontend単独起動で、接続確認はDesktop未接続を表示します。現時点のUIは英語のみで、英日対応は今後の予定です。

[Seed生成の技術契約（英語）](docs/bounded-seed-generation.md)では、大小文字変換・固定prefix/suffix、決定的な経路順序、独立した試行数・配送数上限、キャンセル、プライバシー保証を説明しています。生成処理は構造解析と独立しており、候補が見つからなくても充足不能の証明にはなりません。

## プライバシーと利用範囲

Issue・PR・サンプルデータには実際のパスワード、秘密情報、機密性のある手がかりを投稿しないでください。Cluebenchはローカル完結を原則とし、テレメトリやオンラインのログイン自動試行は想定していません。

## ライセンス

[Mozilla Public License 2.0](LICENSE)（`MPL-2.0`）を採用しています。将来追加する第三者依存コードには、それぞれのライセンスが適用されます。

英語版の正本は[README.md](README.md)です。
