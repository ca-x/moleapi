# MoleAPI 构建与发行要求

状态：用户已要求 GitHub Actions 自动构建服务端、PC 客户端和 Docker。当前完成的是调研阶段与发行目标定义；已提交 Application CI、Server/Desktop Builds 和 Docker Multi-platform 三个工作流。检测到应用 manifest/lockfile/Dockerfile 后运行对应任务；目前应用未发布，各任务明确跳过。只有 Research integrity 会验证调研产物，因此本文不是产品构建成功记录。

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
