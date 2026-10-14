# MoleAPI 构建与发行要求

状态：应用代码已进入 main，GitHub Actions 实际执行服务端、桌面和 Docker 构建。2026-10-13 的 a8eeb65：Application CI、Research integrity 和 Docker 多架构构建成功；Linux x64/arm64、macOS arm64 服务端及 worker 检查成功。修复后平台构建的新增证据见下文。

2026-10-14 构建修复：桌面 FileHandle 继承 Tauri Resource，关闭权限应为 `core:resources:allow-close`；模型生成器的固定哈希要求 `vendor/model-engine/engine.js` 在 Windows 保持 LF（CRLF 哈希与失败日志完全一致）；macOS x64 使用官方 `macos-15-intel` runner，避免 ARM runner 的首次 Rosetta 翻译超过 worker 启动预算。Windows 桌面前次任务因 hosted runner 失联而终止，缺失日志不视作代码成功或失败证明。actionlint 和模拟 core.autocrlf Windows checkout 的模型哈希检查通过；本机缺少 JavaScriptCore/WebKitGTK 4.1，完整桌面验证由新的原生平台 Actions 任务执行。

2026-10-14 原生平台结果：提交 `e9c14cd109ba2f0f0cc6c0e6353d312c8a2256ae` 的 [Server and desktop builds / 38483899443](https://github.com/ca-x/moleapi/actions/runs/38483899443) 已成功完成。独立服务端 Linux x64/arm64、macOS x64/arm64、Windows x64 共5个构建，以及桌面 Linux x64（deb/rpm/AppImage）、macOS x64/arm64（dmg）、Windows x64（NSIS/MSI）共4个打包构建全部成功；每个平台的 worker 检查和 artifact 上传步骤均成功。提交 `746e50a` 的 Application CI / 38487325358 与 Docker / 38487325380 也已成功。这些是对应历史提交的构建证据，不能替代当前主分支检查、实际安装验收、签名或公证；当前主分支的远程构建仍在队列中。

| 产物 | 目标平台/架构 | 安装/分发形式 |
| --- | --- | --- |
| 独立服务端 | Linux x86_64/aarch64 | 压缩包 + 校验和；前端资源嵌入 Rust 二进制 |
| 独立服务端 | macOS x86_64/aarch64、Windows x86_64 | 压缩包 + 校验和 |
| Tauri 客户端 | Linux x86_64，验证后扩展 aarch64 | deb、rpm、AppImage |
| Tauri 客户端 | macOS x86_64/aarch64 | dmg + app |
| Tauri 客户端 | Windows x86_64 | NSIS exe、MSI |
| Docker 服务端 | linux/amd64、linux/arm64 | GHCR 多架构 manifest、版本 tag、sha tag |

CI 分层：

1. pull_request/push 执行 Rust 格式、Clippy、单元/接口测试、前端类型检查/测试/生产构建。
2. 默认分支/手动构建平台矩阵，上传服务端二进制与桌面安装包供验证。
3. v* tag 先完整验证，再生成 GitHub Release 及校验和，发布版本 Docker 镜像。
4. Docker 多阶段先构建本地前端，再编译嵌入资源的服务端；运行镜像不依赖 Node，不需要挂载静态前端目录。SQLite 与用户文件使用持久化数据卷。
5. Tauri frontendDist、字体、CodeMirror 语言包等均为本地产物，不从 CDN 加载。安装包包含独立本地数据存储和执行引擎，不要求另装服务端。
6. macOS/Windows 签名与 Tauri updater 需要真实证书/密钥；没有这些凭据时，构建只能如实标明签名状态，不能宣称已完成公证或签名。发布流程不得输出 secrets。

建议复用成熟工具：tauri-action、docker/setup-buildx-action、docker/build-push-action、docker/metadata-action、Cargo/Rust 缓存、平台原生 runners。要固定工具版本、限制 workflow 权限，发行后的自动更新资产需有可信签名。Docker/Release 流程与功能验收是两件事，工作流成功不证明功能与竞品对齐。
