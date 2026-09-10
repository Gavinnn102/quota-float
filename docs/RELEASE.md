# 发布说明

## 发布目标

Quota Float 同时保留 Windows、macOS 与 Ubuntu 26.04 发布路径：

- Windows：MSI + NSIS；
- macOS：universal app + DMG；
- Ubuntu 26.04 x86_64：原生 `.deb`（Linux 不生成 AppImage）；
- 全部产物：`SHA256SUMS.txt`。

平台通用配置位于 `src-tauri/tauri.conf.json`，Ubuntu 专用窗口和 Debian 配置位于 `src-tauri/tauri.linux.conf.json`。

## Ubuntu 本地构建

使用 Node.js 20.19+ 或 22.12+、Rust stable，并安装与 Ubuntu CI 一致的开发依赖：

```bash
sudo apt update
sudo apt install \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  xwayland
```

再执行：

```bash
npm ci
node scripts/verify-release-version.mjs
npm test
npm audit --audit-level=high
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
export RUSTFLAGS="--remap-path-prefix=$PWD=/src --remap-path-prefix=$HOME=/build"
npm run tauri:ubuntu
```

`npm run tauri:ubuntu` 会使用 Cargo 锁文件构建，随后规范化依赖并解包校验安装树。产物 `Quota Float Ubuntu_<版本>_amd64.deb` 位于 `src-tauri/target/release/bundle/deb/`。标准化和验证脚本按当前版本选择 `.deb`，因此目录中可以安全保留 0.1.9、0.1.10、0.1.11 和 0.1.12 旧版测试包；构建 0.1.12-2 时应保持这些文件不变。

## Windows 与 macOS 构建

在对应主机运行：

```bash
npm ci
node scripts/verify-release-version.mjs
npm test
npm audit --audit-level=high
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run tauri -- build
```

macOS CI 额外安装 `aarch64-apple-darwin` 与 `x86_64-apple-darwin`，并构建 universal 目标。

## GitHub Release

版本文件必须一致且使用尚未占用的标签，例如当前版本：

```bash
git tag v0.1.12-2
git push origin v0.1.12-2
```

发布工作流会：

1. 校验 tag、npm、Cargo、Tauri、双语 README、发布指南和发布模板中的版本与 `.deb` 文件名；
2. 扫描源码中的高置信度密钥；
3. 在 Windows、macOS 与 Ubuntu runner 上执行测试、Clippy 和构建；
4. 对 Windows/macOS bundle 扫描本机构建路径；
5. 解包 Ubuntu `.deb`，扫描实际安装文件树中的禁用文件、高置信度密钥和本机构建路径，并核验依赖与扩展文件；
6. 核验三平台附件，生成 SHA-256，创建 Draft Release。

草稿不会自动公开。实机检查完成后再人工发布。

## Ubuntu 26.04 实机检查

1. 使用 `sudo apt install "./Quota Float Ubuntu_0.1.12-2_amd64.deb"` 安装或升级，确认依赖可解析；首次安装/更新扩展后注销并重新登录。
2. 确认应用在 GNOME 会话启动和正常退出时会尝试启用/禁用扩展，并测试异常退出后的手动禁用与卸载。
3. 多开 ChatGPT，聚焦不同大小的窗口，确认组件始终跟随当前活动窗口。
4. 验证 ChatGPT 移动、缩放、最小化、恢复和工作区切换；拖动或调整大小时确认组件平滑跟随，250 毫秒轮询只作为发现/恢复兜底。
5. 验证 320 × 320 / 100 × 100 两档切换，确认无法拖到中间尺寸；320 × 320 完整面板保持在 GNOME 桌面坐标中距宿主右下角默认 24 px。
6. 在宿主既非全屏也非完全最大化时，验证 100 × 100 紧凑圆球保持右侧 24 个桌面坐标 px，并从默认底边锚点向上移动 136 个逻辑 px；有效几何缩放为 1 的常规逻辑桌面上对应底边 160 px，其他缩放/坐标模式应保持上移距离；宿主全屏或完全最大化后恢复默认 24 px。
7. 验证半屏平铺和单轴最大化仍使用紧凑避让位置；宿主过矮时圆球钳制到宿主顶部，并确认定位只依赖窗口状态和边框几何。
8. 在 fractional scale、跨显示器移动及显示器/工作区变化后，验证组件自身几何换算保持 136 个逻辑 px 的上移距离和默认 24 px 桌面坐标边距。
9. 关闭组件窗口后，确认托盘勾选取消且第一次点击“显示面板”即可恢复。
10. 在未登录、断网和服务错误状态下确认错误页与托盘“立即刷新”均可恢复。
11. 确认托盘不再出现“解锁悬浮窗”，并验证含旧 `locked: true` 字段的偏好升级后窗口仍可交互。
12. 在顶部栏托盘切换“5 小时额度”/“一周额度”，确认仅当前项勾选；验证完整面板和紧凑圆球的百分比、颜色、周期与重置时间同步变化。
13. 重复点击当前周期、切换中英文、退出并重新启动，确认选择始终正确且已保存；旧配置首次升级应保留一周显示。
14. 接口缺少所选窗口时应显示不可用，另一周期仍可切换查看；临近任一窗口重置时应加快刷新。
15. 校验 `sha256sum -c SHA256SUMS.txt`，确认 0.1.9、0.1.10、0.1.11 和 0.1.12 旧版 `.deb` 文件及校验和未变化。

## 跨平台发布门槛

- Windows 宿主跟随、MSI 与 NSIS 构建通过；
- macOS universal app 与 DMG 构建通过；
- Ubuntu `.deb`、GNOME Wayland 锚定（含紧凑圆球的底部避让）和扩展生命周期通过；
- 三平台 CI 全部通过，严重和高风险问题清零。
