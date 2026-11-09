# VoicePaste Personal

适用于 Apple 芯片 Mac 的本地语音输入工具。按快捷键说话，文字会粘贴到当前输入框。默认使用千问 Qwen3-ASR 1.7B 高精度版，也提供 0.6B 轻量版。程序源码和个人词库可以自己保管。

## 下载并使用

1. 在自己的 GitHub 仓库中打开 **Actions → Build Mac app**。如果没有运行记录，点 **Run workflow**。构建完成后，在该次运行底部的 **Artifacts** 下载 `VoicePaste-Personal-AppleSilicon.zip`。
2. 解压，打开 `VoicePaste Personal.app`，按引导开启麦克风和辅助功能。构建默认使用临时签名，个人测试时 macOS 可能要求在「隐私与安全性」确认打开。对外发布时，可以在自己控制的仓库加上 Apple Developer ID 签名与公证。
3. 在 **语音识别模型 → 本地模型** 下载并启用「千问 Qwen3-ASR 1.7B · 高精度」。首次下载要联网，并留出数 GB 的磁盘空间。0.6B 轻量版约 938 MB。模型下载后，使用主快捷键听写可以离线进行。
4. 想用大疆麦克风时，把接收器接到 Mac，在 **系统设置 → 声音 → 输入** 选大疆设备，确认说话时有输入电平，再开始听写。

## 个人词库

打开 **设置 → 热词库 → 个人词库**。

- **识别热词**每行写一个名字或术语，如 `Obsidian`、`Qwen`。这些词随识别会话传给本地千问。
- **固定纠错**每行写一条规则，如 `obsidian → Obsidian`。这类规则在文本输出前执行。
- 点击 **保存并启用**。用 **导出备份** 保存一份 JSON。换电脑时，点击 **导入备份**，核对内容，再点击 **保存并启用**。

应用使用 macOS 的应用数据目录存储词库和模型。在 Finder 按 `⌘⇧G` 可打开 `~/Library/Application Support/com.hululu.voicepaste.personal`。其中 `personal-dictionary.json` 是词库，`models` 是已下载的模型目录。想在另一台 Mac 避免重新下载，可先退出两边的应用，再复制 `models`。词库、录音和模型文件都应留在个人备份中。

## 源码和云端构建

源码基于 [VoicePaste v2.1.6](https://github.com/that-yolanda/voicepaste/tree/v2.1.6)，包括桌面界面、录音和粘贴后端、资源、依赖锁定文件，以及当时用的 `keytap` 源码。独立应用标识和构建流程都已配置。普通听写使用本地千问。原项目的可选在线模型与润色仍可手动配置。

把这个文件夹的内容提交到自己的 GitHub 仓库，保留 `.github/workflows/build-macos.yml`。每次提交到 `main` 或 `master` 时，GitHub 的 Apple 芯片运行器会编译、测试词库逻辑、检查应用签名，再把 `.app` 压成下载包。GitHub 提供标准公开仓库运行器，私有仓库的运行时间受套餐限制。也可以手动运行该流程。

本地维护时需要 Node.js、pnpm、Rust 和 macOS 开发工具。先运行 `pnpm install --frozen-lockfile`，再运行 `pnpm dev`。语音模型登记在 `registry.json`，本地千问的接入在 `src-tauri/src/asr/sherpa_onnx/qwen3_asr.rs`，个人词库在 `src-tauri/src/personal_dictionary.rs`。

## 模型与许可

- 程序保留原始 [MIT 许可证](LICENSE)。按原版本保留作者归属和许可证。
- `vendor/keytap` 来自 [固定提交](https://github.com/that-yolanda/keytap/tree/1be90f9108e0e22fb1a9cff8d5ff4533161a7bc0)，上游声明 MIT 或 Apache-2.0。
- 千问模型采用 [Apache-2.0](https://huggingface.co/Qwen/Qwen3-ASR-1.7B)，本项目使用 [sherpa-onnx 提供的 ONNX 模型包](https://k2-fsa.github.io/sherpa/onnx/qwen3-asr/pretrained.html)。模型在首次运行时单独下载。可以自行备份已下载模型，不需要 VoicePaste 网站或订阅才能用本地听写。

常规构建和词库测试无需录音。可选的上游语音集成测试使用独立录音样本，需要测试者自行准备。
