# Quota Float

[English](README.md) | [简体中文](README.zh-CN.md)

一款跨平台 Codex 额度桌面组件，读取本机已有的 Codex 登录态，显示剩余额度、重置时间和重置机会。

![Quota Float 配额状态](docs/images/quota-states.png)

## 平台支持

- **Windows：**保留原有 ChatGPT/Codex 宿主窗口跟随实现，并生成 MSI 与 NSIS 安装包。
- **macOS：**生成同时支持 Apple Silicon 与 Intel 的通用 App 和 DMG。
- **Ubuntu 26.04 x86_64：**生成原生 `.deb`，并通过随包 GNOME Shell 48–50 扩展在 Wayland 下锚定到活动 ChatGPT 窗口。

Ubuntu 是新增的平台路径；共享的 React 界面、额度读取、偏好、托盘和发布逻辑仍支持 Windows 与 macOS。

## 显示内容

- Codex 套餐、5 小时或一周剩余额度、对应的下次重置时间，以及接口提供时的重置机会信息。
- 两种额度周期的完整面板均在重置日期行和重置机会之间显示“**点数余额**”（英文 **Credits**），金额在中英文界面统一使用 `US$48.44` 格式，并随额度刷新更新。美元显示按每 25 点折合 US$1 换算，不代表折扣购入成本；余额缺失显示 `—`。换算边界见[已知限制](docs/KNOWN-LIMITATIONS.md)。
- 正常、提醒、紧急、过期、未登录和暂不可用等状态。
- 320 × 320 完整面板和 100 × 100 紧凑额度圆球。
- 持久保存面板显示状态、完整/紧凑模式、额度周期和界面语言。
- 大窗口中的 **5h**/**1w** 徽标可在 5 小时额度页和一周额度页之间点击切换，外观保持不变。顶部栏托盘菜单提供“5 小时额度”与“一周额度”两个选项，勾选表示当前周期；完整面板和紧凑圆球同步切换，重启后保留选择。旧配置默认显示一周额度。
- 托盘还提供显示/隐藏、刷新、语言、开机启动和退出控制。
- 错误页与托盘均提供手动刷新，自动退避不会阻断恢复入口。

## Ubuntu 安装

下载 Ubuntu 26.04 的 `.deb`，使用 `apt` 安装以自动解析运行时依赖：

```bash
sudo apt install "./Quota Float Ubuntu_0.1.12-2_amd64.deb"
```

正式支持的 Linux 桌面会话是 Ubuntu 26.04、GNOME Shell 48–50 与 Wayland。请先在同一台电脑登录 Codex。首次安装或更新扩展后，请注销并重新登录一次，让 GNOME Shell 扫描扩展。在受支持的 GNOME 会话中，Quota Float 会在启动时尽力启用扩展，并在正常退出时尝试禁用；若系统策略、崩溃或强制结束导致清理未执行，可手动控制：

```bash
gnome-extensions disable quota-float-anchor@quotafloat.app
gnome-extensions enable quota-float-anchor@quotafloat.app
```

卸载会移除应用和扩展文件：

```bash
sudo apt remove quota-float-ubuntu
```

Debian 包依赖 `libwebkit2gtk-4.1-0`、`libgtk-3-0t64`、`libayatana-appindicator3-1` 和 `gnome-shell (>= 48)`；随包扩展的元数据当前支持 GNOME Shell 48–50。

## GNOME Wayland 窗口锚定

Wayland 不允许普通应用强制设置跨应用坐标，因此 Quota Float 只把定位交给本地 GNOME Shell 扩展。扩展优先选择当前聚焦的 ChatGPT 窗口，用户操作组件时继续沿用刚才的宿主窗口。320 × 320 完整面板始终在 GNOME 桌面坐标中距宿主右下角 24 px。对于 100 × 100 紧凑额度圆球，当宿主既未全屏也未完全最大化时，右侧保持 24 个桌面坐标 px，并从默认的 24 px 底边锚点向上移动 136 个逻辑 px，为宿主底部输入区和发送/语音按钮留出空间；在有效几何缩放为 1 的常规逻辑桌面上，这对应底边 160 px，其他缩放和坐标模式会根据组件自身的有效几何缩放换算，保持 136 个逻辑 px 的上移距离。宿主重新全屏或完全最大化后，紧凑圆球恢复默认的 24 px 桌面坐标底边距。半屏平铺和单轴最大化的宿主继续使用紧凑模式的避让位置；宿主过矮时会将圆球钳制到宿主顶部。

定位和更新只根据宿主窗口的边框几何以及全屏/最大化状态计算。宿主移动或调整大小时，扩展监听位置、尺寸和状态变化，并将更新合并到下一次 GNOME 重绘；250 毫秒发现/恢复检查仍作为兜底。扩展只跟随当前宿主与组件，宿主切换、组件销毁或扩展禁用时会清理订阅和待执行的帧回调。不读取窗口内容，也不进行像素识别。

Linux 窗口在技术上保持可调整，以便 GTK 接受程序化模式切换；但原生最小/最大尺寸会固定为当前的 320 × 320 或 100 × 100，只允许这两档，不允许手动调整到中间尺寸。

为识别目标，扩展会检查 GNOME 顶层窗口列表中的有限应用身份、焦点、工作区、最小化状态和边框几何；不会读取窗口内容，也不会联网。完整边界与启停生命周期见 [PRIVACY.md](PRIVACY.md)。

## 额度读取方式

Quota Float 只读取已有的本机登录文件：

- 已设置 `CODEX_HOME` 时：`$CODEX_HOME/auth.json`；
- 否则：`~/.codex/auth.json`。

它仅使用已有会话调用 Codex/ChatGPT 额度接口；不会按本地 Token 数估算额度、兑换重置机会、修改账户设置或保存凭据。浏览器预览使用模拟数据；真实额度需要 Tauri 桌面应用和本机 Codex 登录态。

5 小时和一周额度分别匹配接口返回的窗口时长。所选周期未返回时，组件会提示该周期不可用；不会用另一周期的百分比代替。

## 开发

准备 Node.js 20.19+ 或 22.12+、Rust stable 和项目依赖后执行：

```bash
npm ci
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run tauri -- dev
```

Ubuntu 26.04 还需要 [发布说明](docs/RELEASE.md) 中列出的 Tauri GTK/WebKit 开发依赖。

## 构建

在 Windows 或 macOS 主机上构建对应原生包：

```bash
npm run tauri -- build
```

构建 Ubuntu Debian 包：

```bash
export RUSTFLAGS="--remap-path-prefix=$PWD=/src --remap-path-prefix=$HOME=/build"
npm run tauri:ubuntu
```

该命令会使用已提交的 Cargo 锁文件构建、规范化运行时依赖字段，并解包校验安装内容。产物 `Quota Float Ubuntu_<版本>_amd64.deb` 位于 `src-tauri/target/release/bundle/deb/`，与已有的 0.1.9、0.1.10、0.1.11 和 0.1.12 Debian 测试包并存；旧包保持不变。路径重映射用于确认安装包没有嵌入本机构建路径。Linux 刻意选择 `.deb` 而非 AppImage；这项 Linux 专用选择不会取消 Windows 或 macOS 产物。

## 发布

CI 覆盖共享前端以及 Windows、macOS、Ubuntu 桌面构建。推送匹配版本的 `v*` 标签后，草稿 Release 应包含：

- 含一个 MSI 和一个 NSIS 的 Windows 压缩包；
- 含一个通用 App 和一个 DMG 的 macOS 压缩包；
- 一个 `Quota Float Ubuntu_<版本>_amd64.deb`；
- `SHA256SUMS.txt`。

Ubuntu 发布门禁会先解包 `.deb`，再扫描实际安装树中的禁用文件、高置信度密钥和本机构建路径。发布前请阅读 [docs/RELEASE.md](docs/RELEASE.md)。

## 许可证

MIT
