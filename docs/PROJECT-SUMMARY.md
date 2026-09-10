# Quota Float 项目简介

## 定位

Quota Float 是一个跨平台 Tauri 2 Codex 额度组件。它通过本机已有登录态只读查询额度，并用卡片或紧凑圆球显示 5 小时或一周额度、重置时间、重置机会和套餐类型。

完整面板另显示点数余额（英文 Credits），复用额度接口的 `credits.balance`，按 25 点/美元换算为两种语言一致的 `US$48.44` 格式并随刷新更新；无新增网络请求。该固定换算口径的边界见[已知限制](KNOWN-LIMITATIONS.md)。

## 平台架构

- 共享层：React 19、TypeScript、Vite、Rust、`reqwest`、偏好、托盘和额度解析。
- Windows：`src-tauri/src/codex_host.rs` 负责原生 ChatGPT/Codex 宿主跟随；发布 MSI/NSIS。
- macOS：使用共享窗口路径；发布 universal app/DMG。
- Ubuntu 26.04：`src-tauri/src/ubuntu_host.rs` 与 `src-tauri/gnome-extension/` 负责 GNOME Wayland 尺寸和锚定；发布 `.deb`。

平台通用 Tauri 配置位于 `src-tauri/tauri.conf.json`；Linux 专用覆盖位于 `src-tauri/tauri.linux.conf.json`，因此 Ubuntu 支持不会替换其他平台。

## Ubuntu 行为

- 扩展选择当前聚焦的 ChatGPT 窗口；组件获得焦点时保留刚才的宿主。
- 身份字段逐项识别，可处理多个字段同时包含 `codex` 的窗口。
- 组件只允许 320 × 320 和 100 × 100 两种原生尺寸。
- 320 × 320 完整面板保持在 GNOME 桌面坐标中距宿主右下角 24 px。100 × 100 紧凑圆球在宿主既非全屏也非完全最大化时，右侧保持 24 个桌面坐标 px，并从默认底边锚点向上移动 136 个逻辑 px，为宿主底部输入区和发送/语音按钮预留空间；有效几何缩放为 1 的常规逻辑桌面上对应底边 160 px，其他坐标模式按组件自身几何换算。全屏或完全最大化恢复默认 24 px，半屏平铺和单轴最大化继续避让，宿主过矮时钳制到顶部。
- 宿主位置、尺寸和状态变化会在下一次 GNOME 重绘前合并处理；250 毫秒发现/恢复轮询作为兜底，宿主切换、组件销毁和扩展禁用时清理旧订阅与待执行回调。
- 紧凑窗口定位只使用宿主边框几何和全屏/最大化状态，不读取窗口内容或进行像素识别。
- 关闭窗口会持久化隐藏状态并同步托盘勾选；首次点击“显示面板”即可恢复。
- 应用在 GNOME 会话启动时尽力启用扩展，并在正常退出时尝试禁用；扩展仅操作 Quota Float 窗口。
- 顶部栏托盘提供 5 小时/一周额度选择，两个视图同步切换并保存周期；旧配置默认一周。另有显示/隐藏、刷新、语言、开机启动和退出；旧配置中的 `locked` 字段会被忽略并在后续保存时移除。

## 恢复与发布门禁

- 错误页和托盘均提供手动刷新，绕过最长 30 分钟自动退避等待。
- Windows、macOS 与 Ubuntu CI 均保留。
- Ubuntu `.deb` 在上传前会被解包，扫描实际安装树中的禁用文件、密钥模式和本机构建路径。
- 当前版本为 0.1.12-2，构建时保留已有 0.1.9、0.1.10、0.1.11 与 0.1.12 Debian 测试包。

## 数据和安全边界

- 认证文件来自 `CODEX_HOME/auth.json` 或 `~/.codex/auth.json`。
- 不保存 Token、账户 ID、提示词、聊天记录、原始响应或认证文件路径。
- 认证文件读取上限 256 KB，接口响应上限 1 MB；请求不跟随重定向。
- 不兑换重置机会，不修改账户设置。

## 验证命令

```bash
node scripts/verify-release-version.mjs
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
export RUSTFLAGS="--remap-path-prefix=$PWD=/src --remap-path-prefix=$HOME=/build"
npm run tauri:ubuntu
```
