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


## 随发行包交付 Java 运行时

正式服务端归档、桌面安装包和 Docker 构建现在使用固定的 Eclipse Temurin
JRE21.0.12.1+1。构建脚本按目标平台核对原始包大小/SHA256，并保留全部上游 legal
文件及对应源码版本/下载信息；应用运行时不下载 JRE，也不搜索 PATH。

```sh
python scripts/prepare_openapi_generator.py
python scripts/prepare_java_runtime.py --target x86_64-unknown-linux-gnu
# 默认本机桌面构建会准备这两个资源，再传入生成的资源配置。
npm --prefix web run desktop:build
```

交叉构建必须先为实际目标准备 JRE，再在 desktop 目录向 Tauri 传入
`--config tauri.java.json --target <目标>`。同一工作树一次只准备一个目标。
桌面 release 构建拒绝缺失资源配置或与 TARGET 不一致的 JRE；缓存/下载文件不进 Git。
服务端归档包含 executable 与 runtime/java，需一起解包。桌面用 Tauri resource_dir
读取对应资源，Docker 使用固定 /opt/moleapi/runtime/java/payload/bin/java。
MOLEAPI_CODEGEN_JAVA 与已有桌面用户选择可覆盖默认自带 JRE。

Linux AppImage 的 `linuxdeploy` 会改写 JRE 内 ELF 文件，导致固定原始清单哈希失效。桌面构建在 Tauri 打包后调用 `scripts/repair-linux-appimage-java.py`：从固定源恢复完整 JRE，使用 SHA256 固定的 `linuxdeploy-plugin-appimage` 内 `appimagetool` 和原 AppImage runtime 重新封装，解包复核 JRE、worker 与 Dubbo。脚本拒绝修改已签名的 AppImage；启用签名时必须在重新封装后签名。旧构建 `38946817315` 的真实包已在本机按同一流程重新封装并通过这三项检查，当前分支的原生 CI 仍待结果。

CI 为无 FUSE 环境全局设置 `APPIMAGE_EXTRACT_AND_RUN=1`。修复脚本读取 AppImage runtime 的签名和偏移时临时移除该变量，避免把 runtime 查询误当成应用启动；实际解包步骤仍显式启用它。run `38963507351` 首次执行修复脚本时在签名查询处返回101，本地用相同环境复现并验证修复；该次打包结果不算通过。

分支构建 `38972262886` 已产出真实 Linux deb/rpm/AppImage，独立 deb 解包、JRE 原始清单与 Dubbo worker 自检通过，但 Ubuntu 22.04 上的 `rpm2cpio` 对该 RPM 返回1且没有错误正文，导致 RPM 与 AppImage 后续自检未执行。下载该次构建的原始 RPM 后，本地 `bsdtar` 可完整列出并解包；解包的 JRE 清单与 Dubbo worker 校验通过。本地 Arch 缺少 Ubuntu 打包程序所需的 WebKitGTK4.1 动态库，不能替代原生桌面 worker 检查。自检脚本已改为使用 `libarchive-tools` 的 `bsdtar` 提取真实 RPM；仅在 GitHub Linux runner 缺少该命令时，利用工作流先前更新的 apt 索引安装此工具。本地缺少工具会明确报错。新的原生 CI 结果仍待验证。

经典 Dubbo 使用同一份固定 Temurin JRE 和独立固定哈希的 Apache Dubbo worker JAR。构建前运行 `python scripts/prepare_dubbo_worker.py`（首次需要显式 Maven/JDK 或提供已构建且哈希匹配的 JAR）；独立服务端压缩包和桌面资源均放在 `runtime/dubbo/engine.jar`，Docker 固定路径为 `/opt/moleapi/runtime/dubbo/engine.jar`。`scripts/check-bundled-dubbo.py` 检查 JAR/JRE 原始字节并实际启动 worker；真实业务调用、原生安装与容器 smoke 仍以对应平台流水线为准。开发者可显式设置绝对路径 `MOLEAPI_DUBBO_WORKER_JAR`，运行时不会下载或接受工作区提供的可执行文件路径。

发行验证用 scripts/check-bundled-java.py：在清空外部 Java 配置、使 PATH 不可用后，
启动真实独立服务端，通过归属校验 API 生成 Go 文件并检查原始规范未变。
`--runtime-only` 可检查桌面打包资源与实际 JVM。Docker CI 同时执行真实 JRE/生成 API
验收。Linux 本机归档/原生界面已验证；其余 OS、安装包实际安装、容器新提交、签名/
公证以及对应源码的最终发布仍按原有发布门禁确认，不能从资源清单推断通过。


2026-10-15 容器原生结果：main ea34ce1 的 Docker run38698102016 已完成 linux/amd64、
linux/arm64 和 manifest。两个原生镜像均通过固定 JRE21.0.12.1+1 启动及真实归属 API
生成（每个17个 Go 文件、原始规范保持不变），并通过嵌入前端/worker 检查。
这只证明该提交对应双架构容器；后续提交、桌面安装包和源码/签名发布门禁仍独立确认。
