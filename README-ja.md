# Cluebench

Cluebenchは、断片的・不確実なパスワードの手がかりを整理し、複数の解釈や仮説を比較しながら、制約と候補生成戦略へ結び付けることを目指すローカル完結型のデスクトップアプリケーションです。

CTF、本人のパスワード復旧、明示的に許可されたセキュリティ分析を主な用途とします。パスワードの検証・クラッキングエンジン自体を提供したり、オンライン認証への自動試行を行ったりすることは目的としません。

> **現在の状況：** Rust Coreに、ドメイン検証・仮説の有効化・候補評価、限定的な制約解析と上限付き候補生成を実装しています。Desktopでは、明示的な大小文字変換・固定prefix/suffixに加え、独立したDirect ConstraintsとClueに基づくHypothesesを編集できる手動seed候補プレビューを提供します。Clueの原文・出典と複数の解釈を分けて保持し、型付きConstraint effectsを追加できます。Hard条件は候補を絞り込み、Soft Hypothesisは全effectsが一致した場合に確信度の点数を一度だけ加算します。結果から送信時の解釈と元のClueを追跡できます。合成データによるCore接続確認も補助機能として残しています。完全なClue/Hypothesisワークスペース・wordlist出力・永続化と残りのMVPワークフローは、まだ実装・リリースされていません。仕様は[Previewの技術契約（英語）](docs/bounded-candidate-preview.md)を参照してください。

[Previewの検証報告（英語）](docs/candidate-preview-verification.md)と[以前のShellの検証報告（英語）](docs/desktop-verification.md)は、作成時点のローカル検証証拠であり、現在のCI状況を示しません。CI実行結果の正式な記録は[GitHub Actions](https://github.com/cludewn/cluebench/actions)です。対象commitと各jobを直接確認してください。Native compilation、実GUI実行、IPC smoke検証は別々の確認です。変更ごとのローカルテスト・手動検証・未実施項目・制約は、該当PRへ記載します。

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

Previewは、1行につき1件のprintable ASCIIの手動seedを受け付け、前後のSpaceや重複行を保持します。大小文字変換・順序付きprefix/suffixを選び、明示的にPreviewを実行します。最大16件の独立したDirect Constraintsに加え、最大8件のClueと8件のClue-backed Hypothesisを編集できます。両方の方式でLength・Position・Contains・NotContains・Prefix・Suffix・CharacterCountの7種を使用します。Hypothesisには解釈文、元のClue、有効状態、Hard/Soft、確信度と最大4件のeffectsを設定できます（Clue-backed effectsは合計16件まで）。Hardでは全effectsの一致が必要です。Softでは全effectsが一致したとき、Low 1・Medium 2・High 3の相対点をHypothesis単位で一度だけ加算します。点数は確率を表さず、候補順も変えません。無効化中も全draftの検証が必要です。ゼロeffectsのHypothesisは残せますが、絞り込みや加点を行いません。Direct Constraintsは従来どおり独立したManual originとして扱います。

Clue原文・出典・解釈文は別々のUnicode textareaで編集します。空文字、前後の空白、LF、TAB、結合文字、Emojiをtrimや正規化なしで保持し、CR・孤立UTF-16 surrogate・個別byte上限超過を拒否します。Paste・Drop・native beforeinputで観測できたCR/CRLFはDOM正規化前に操作全体を拒否しますが、ブラウザが先に正規化した原文byteは復元できません。Clueを参照するHypothesisが1件でもあれば、無効化中や未完成のdraftでもClueは削除できません。先にHypothesisを削除するか、別のClueへ明示的に付け替える必要があります。canonical request全体で16 KiBの共有予算があるため、すべての個別上限を同時に使えるとは限りません。

既定の試行上限は2,048 Path、出力上限は50 recordで、最大値はそれぞれ4,096と100です。ローカルRaw IPCを通じ、単一のblocking workerで生成します。編集とClearは表示を無効化しますが、Rustの生成処理は取り消しません。Softの一致説明には、送信したrequestのスナップショットと検証済みCore/editor ID対応を使い、元のClue原文・出典まで表示します。入力・結果をログ出力・保存・外部送信しませんが、メモリの安全な消去は保証しません。`pnpm -C apps/desktop dev`はFrontendのみを起動し、ネイティブ操作はDesktop未接続を表示します。現在のUIは英語です。

[Seed生成の技術契約（英語）](docs/bounded-seed-generation.md)では、大小文字変換・固定prefix/suffix、決定的な経路順序、独立した試行数・配送数上限、キャンセル、プライバシー保証を説明しています。生成処理は構造解析と独立しており、候補が見つからなくても充足不能の証明にはなりません。

## プライバシーと利用範囲

Issue・PR・サンプルデータには実際のパスワード、秘密情報、機密性のある手がかりを投稿しないでください。Cluebenchはローカル完結を原則とし、テレメトリやオンラインのログイン自動試行は想定していません。

## ライセンス

[Mozilla Public License 2.0](LICENSE)（`MPL-2.0`）を採用しています。将来追加する第三者依存コードには、それぞれのライセンスが適用されます。

英語版の正本は[README.md](README.md)です。
