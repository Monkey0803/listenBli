# listenBli

用 Rust 写的哔哩哔哩音乐播放器：搜索 B 站上的音乐/视频，播放其音频，并同步滚动显示歌词。
支持 **macOS** 与 **Windows**，未登录即可使用，登录后可获得更高音质与「我的」内容。

```
┌──────────────────────────────────────────────────────────────┬────────────┐
│ ♫ listenBli  [搜索|收藏夹|历史]  ( ⌕ 搜索歌曲…     ⌘K ) ✦无损优先 ⚙ 扫码登录 │
├──────────────────────────────────────────────────────────────┤   ☰ 歌词    │
│ 「周杰伦」找到 12 个结果                        ▶ 全部播放      │  ⊙跟随 ⇄翻译 │
│  01 ▣ 【4K修复】周杰伦 - 晴天        [192K] 4:29 ⋯            │  来源：B站CC │
│  02 ▣ 晴天 - 周杰伦 官方MV                 4:29               │             │
│  03 ▣ 【Hi-Res】周杰伦 经典歌曲合集 50首  1:15:32             │  当前行高亮  │
├──────────────────────────────────────────────────────────────┤  自动居中   │
│ ▣ 晴天 周杰伦·192K AAC-LC ⏮ ▶ ⏭ ──●───── 01:12/05:17 ≡ 🔊 ── │  点击跳转   │
├──────────────────────────────────────────────────────────────┴────────────┤
│ ● 未登录（可正常听歌；登录后可获取更高音质与「我的」内容）    配置：~/... ⧉  │
└───────────────────────────────────────────────────────────────────────────┘
```

界面按 [`designs/listenbli-player/`](designs/listenbli-player/) 的「深夜电台」设计稿实现：
分层炭灰底 + B 站粉作为唯一的「正在播出」信号色，全部控件为矢量绘制，不依赖系统控件外观。

## 功能

- **搜索与播放**：关键词搜索 B 站视频，点击即播放其音轨（无视频画面，纯音频播放）。
- **搜索结果自动翻页**：列表滚动到接近底部时自动拉取下一页，无需点按（加载中在列表末尾
  显示一个小转圈），并按 bvid 去重（B 站第 2 页会重复第 1 页的少量视频），到末页自动停下。
- **搜索记录**：最近的 8 条搜索会写回配置，重启后仍在；聚焦空搜索框时下拉列出，
  也可以点击「清空搜索记录」清空。
- **行内更多操作**：悬停列表行时行尾出现「⋯」——复制/打开视频链接、加入播放列表、
  下一首播放、只缓存不播放。
- **未登录可用**：匿名即可获取 **192K AAC-LC** 音质。
- **扫码登录**：使用哔哩哔哩手机客户端扫码，凭据（`SESSDATA` 等）加密落盘、重启后保留。
- **歌词显示**：自动跟随高亮、自动居中滚动、点击歌词行跳转、支持原文/翻译切换。
- **我的内容**（登录后）：收藏夹、观看历史。
- **播放控制**：播放/暂停、上一首/下一首、进度拖动、音量、自动连播。
- **无损优先**（可选）：大会员账号可优先使用 FLAC 无损流。
- **外观设置**：强调色（信号粉 / 霓虹青 / 夜紫）、歌词字号、列表密度、中文字体，
  全部即时生效并写回配置。
- **快捷键**：`空格` 播放/暂停、`←`/`→` 快退/快进 5 秒、`⌘K` 聚焦搜索、`Esc` 关闭浮层。

## 安装与运行

### 前置条件

**macOS**：安装 Rust 工具链即可，无需任何额外系统依赖。

```bash
curl https://sh.rustup.rs -sSf | sh
```

**Windows**：需要 Rust 工具链 **以及 MSVC 链接器**。

1. 安装 [Visual Studio Build Tools 2022](https://visualstudio.microsoft.com/visual-cpp-build-tools/)，
   勾选 **「使用 C++ 的桌面开发」**（提供 `link.exe` 与 Windows SDK）。
2. 安装 Rust：

```powershell
winget install Rustlang.Rustup
# 或从 https://sh.rustup.rs 下载 rustup-init.exe
```

> `x86_64-pc-windows-msvc` 是推荐目标。`x86_64-pc-windows-gnu`（MinGW）理论上可行，但非首选。

### 构建与运行

```bash
git clone https://github.com/Monkey0803/listenBli.git && cd listenBli
cargo run --release
```

首次构建需要编译 `wgpu`、`symphonia` 等依赖，耗时较长属正常。

### 打包成 macOS 应用（.app）

```bash
./scripts/bundle-macos.sh              # 本机架构，产出 dist/ListenBli.app
./scripts/bundle-macos.sh --universal  # arm64 + x86_64 通用包（需先装 x86_64 target）
open dist/ListenBli.app
```

脚本会做四件事：`cargo build --release`、生成图标（`scripts/make-icns.py` → `assets/ListenBli.icns`）、
按 `packaging/macos/Info.plist.in` 组装 bundle、用 `codesign -` 做**临时签名**。

产物是标准的单文件应用，可以直接拖进「应用程序」目录双击运行（16 MB 左右）。
两个注意点：

- **未做公证**：本地构建的 app 不受 Gatekeeper 拦截；但如果被压缩、下载或经 AirDrop 传输，
  会被打上隔离属性而提示「已损坏」。清除方式：

  ```bash
  xattr -dr com.apple.quarantine /Applications/ListenBli.app
  ```

  要正式分发，请用 Developer ID 证书签名并公证：
  `CODESIGN_IDENTITY="Developer ID Application: …" ./scripts/bundle-macos.sh`，
  再走 `xcrun notarytool submit` + `xcrun stapler staple`。
- 配置与缓存仍写在用户目录（见下表），不会随 app 一起移动，因此升级 app 不会丢登录态。

| 环境变量 | 用途 |
|---|---|
| `ARCH` | 单架构构建时强制 `arm64` / `x86_64` |
| `BUNDLE_ID` | 覆盖 `CFBundleIdentifier`（默认 `com.listenbli.app`） |
| `MIN_MACOS` | 覆盖 `LSMinimumSystemVersion`（默认 `11.0`） |
| `CODESIGN_IDENTITY` | 用真实证书签名而不是临时签名 |

### Windows 的图标

两处来源，各管一半：

- **exe 自身的图标**（资源管理器、快捷方式）：`assets/icon.ico`（由 `scripts/make-icns.py`
  一并生成）在构建 Windows 目标时由 [`build.rs`](build.rs) 写进 PE 资源段。首选
  `rc.exe`（Windows SDK，装了「使用 C++ 的桌面开发」自带），其次 `llvm-rc`、`windres`；
  一个都找不到时只打 `cargo:warning`，不中断构建，所以 macOS 上做 Windows 目标的类型级
  `cargo check` 依然可用，只是产不出图标。
- **窗口与任务栏图标**：运行时由 `platform::app_icon()` 把 `assets/icon.png` 解成 `IconData`
  交给 eframe；不给的话 eframe 会装上它内置的 egui 占位图标（黑六边形）。
  macOS 反过来：什么都不设，交给 `Contents/Resources/ListenBli.icns`。

## 使用说明

1. 在顶部搜索框输入歌名/关键词，回车或点「搜索」。搜索过的关键词会被记住：再次点开
   空的搜索框即可从下拉的搜索记录里挑一条重搜。
2. 点击任意结果开始播放。音频会先下载到本地缓存（通常 1–7 MB），随后立即播放。
   结果不止一页时，滚到列表底部会自动加载下一页。
3. 鼠标移到某一行时行尾出现「⋯」，点开是这一行的更多操作：复制视频链接（`bilibili.com/video/…`）、
   在浏览器打开、加入播放列表、下一首播放、缓存到本地（只下载不打断当前播放，完成后弹一条提示）。
4. 右侧歌词面板自动加载并滚动；点击任意歌词行可跳转到该处。
5. 点「扫码登录」用手机 B 站客户端扫码，即可解锁更高音质、收藏夹与历史记录。
6. 点右上角齿轮打开设置：音质、歌词来源、外观与中文字体都在这里调整。
7. 播放条右侧的「播放列表」按钮展开当前队列，点任意一行即可跳转。

## 平台差异

所有平台相关代码集中在 [`src/platform.rs`](src/platform.rs)，其余代码不含平台分支。

| 项目 | macOS | Windows |
|---|---|---|
| 配置目录 | `~/Library/Application Support/listenBli/` | `%APPDATA%\listenBli\config\` |
| 缓存目录 | `~/Library/Caches/listenBli/` | `%LOCALAPPDATA%\listenBli\cache\`（可在设置里改，见 `cache_dir`） |
| 音频后端 | CoreAudio | WASAPI |
| TLS | Security.framework | SChannel |
| 中文字体 | PingFang → Hiragino Sans GB → STHeiti → Songti → Arial Unicode | `%SystemRoot%\Fonts` 下 msyh → simhei → simsun → DengXian |
| 凭据权限 | `chmod 0600` | 依赖 `%APPDATA%` 的按用户 ACL |

- Windows release 构建带 `windows_subsystem = "windows"`，不会弹出控制台窗口。
- Windows 字体路径通过 `SystemRoot` 环境变量解析，不硬编码 `C:\Windows`（系统可能装在别的盘）。
- 若自动探测不到中文字体，界面中文会显示为方块。此时在配置文件的 `cjk_font_path`
  中填入字体绝对路径即可。

## 配置文件

`config.json`（位置见上表）：

```json
{
  "version": 1,
  "volume": 0.8,
  "prefer_flac": false,
  "prefer_translation": true,
  "search_history": ["周杰伦 晴天", "米津玄師"],
  "cjk_font_path": null,
  "cjk_font": "auto",
  "accent": "pink",
  "lyric_size": 15.5,
  "density": "comfortable",
  "cache_dir": null,
  "cookies": { "domains": { "bilibili.com": { "SESSDATA": "…" } } }
}
```

| 键 | 取值 | 说明 |
|---|---|---|
| `cjk_font` | `auto` / `pingfang` / `hiragino` / `stheiti` / `songti` / `yahei` | 界面中文字体；`cjk_font_path` 优先级更高 |
| `accent` | `pink` / `cyan` / `violet` | 强调色 |
| `lyric_size` | `14` – `20` | 歌词字号（px） |
| `density` | `comfortable` / `compact` | 列表行高（62 / 52 px） |
| `search_history` | 字符串数组，最多 8 条 | 最近的搜索关键词，最新的在前；重复搜索会提到最前 |
| `cache_dir` | 绝对路径 / `null` | 缓存（音频分片与歌词）位置；`null` 用系统默认。与配置文件位置**互相独立**，改它不会动配置文件；改动只影响之后写入的文件，已有文件不会搬走 |

> ⚠️ `cookies` 字段等同于账号凭据，请勿分享该文件。

## 工作原理

### 音频链路（边下边播）

1. `x/web-interface/view` 解析 `bvid → cid` 与时长。
2. `x/player/playurl`（WBI 签名）取 DASH 音频流。
3. 选择音质：**优先 `30280`(192K)/`30232`(132K) 这类 AAC-LC**；
   `30216` 是 HE-AAC（`mp4a.40.5`），Symphonia 不做 SBR 上采样，仅在无其它可选时使用。
   Dolby（EC-3）无法解码，直接跳过。
4. 带 `Referer`/UA 发起一次 GET，**边收边写入缓存文件**。
5. 累计到 64 KB 就把播放权交给 UI；`rodio` → `symphonia`（isomp4 + AAC）开始解码。
6. 后台继续下载直到字节数与 `Content-Length` 一致，缓存条目才算完整。

实测（192K / 4:30 的歌，6.2 MB）：

| | 首声延迟 |
|---|---|
| 先下完再播（旧实现） | **4.04 s** |
| 边下边播（现实现） | **0.32 s** |
| 打开解码器耗时 | 0.004 s |

> **为什么需要专门的读取器**：B 站的 DASH 音频是 fragmented MP4
> （`ftyp` + `moov(mvex,trex)` + `sidx` + 54 个 `moof/mdat`）。看起来"下载进文件、
> 边写边播"就够了，但**实测不行**：rodio 的 `Decoder` 在打开时就固定了流长度，
> 部分写入的文件结尾会被当成流的真实结尾。
> [`examples/spike_streaming.rs`](examples/spike_streaming.rs) 复现了这一点——
> 解码器打开后追加 5.99 MB，多解出一个字节的音频都没有。
>
> 解决办法是在 [`src/audio/stream.rs`](src/audio/stream.rs) 实现一个
> `Read + Seek` 视图：**一开始就上报 `Content-Length` 给出的真实总长度**，
> 并让 `read()` 在数据未落盘时用 `Condvar` **阻塞等待**而不是返回 EOF。
> 于是解码器只在需要时才等待字节，而不是一开始就等整首歌。
>
> 三个必须处理的边界：
> * 下载失败/中断 → `fail()`，阻塞中的 `read` 立刻返回错误而不是永久挂起；
> * 下载线程死亡 → 读取有 15 秒超时兜底；
> * 半成品缓存 → 用 `.meta.json` 记录期望总长，**只有长度完全一致才算缓存命中**，
>   否则会被当成完整歌曲播放（截断）。

> **实现要点**：Symphonia 完整支持这些盒子，但**忽略 `tfdt`**；由于下载的是单个从
> t=0 起连续的完整音轨，时间戳仍然正确。另需注意 fMP4 的 `total_duration()` 返回
> `None`，因此**时长一律取自接口返回的 `timelength`**。
>
> **跳转的取舍**：`Player::try_seek` 会阻塞调用线程直到解码器完成定位。对仍在下载的
> 音频，向前跳转到尚未缓冲的位置会等待字节。由于跳转发生在 UI 线程上，我们**先检查
> 缓冲进度，超出则拒绝并提示"音频仍在缓冲"**，而不是让窗口卡死。缓冲只需几秒即可
> 完成，此后任意位置都能立即跳转。

### 请求签名（WBI）

`search/type`、`ranking`、`history` 等接口需要 WBI 签名，否则返回 HTML 风控页或 `-352`：

1. `nav` 返回两张图片 URL，取其文件名作为 `img_key` / `sub_key`；
2. `sub_key + img_key`（64 字符）经固定表重排后取前 32 位，得到 mixin key；
3. 参数加 `wts`、按 key 排序、剔除 `!'()*`、按 `encodeURIComponent` 语义百分号编码，
   拼接 mixin key 后 MD5 得 `w_rid`。

> `nav` 在未登录时返回 `code: -101`（账号未登录）但 **`data` 里仍有 WBI 密钥**，
> 因此该接口必须容错解析——否则匿名用户的签名会全部失效，进而无法播放。

### 歌词来源

按顺序回退，任一环节缺失都不会影响播放：

1. **B 站 CC/AI 字幕**（`x/player/v2` → `subtitle_url`）——忠于视频时间轴，但实测绝大多数
   音乐视频没有字幕。
2. **网易云音乐**（第三方非官方接口）——搜索匹配后取 LRC，并合并 `tlyric` 翻译。

匹配策略（B 站投稿标题噪声极多，这部分是歌词可用性的关键）：

- 标题清洗：删除 `【】` 内的装饰（`【4K修复】`），但**保留** `《》`/`（）` 内的内容
  （歌名常写在里面）；分隔符转空格而**不截断**（否则 `｜《晴天》- 周杰伦` 会被清空）；
  还原数学字母/全角等「花体字」（`𝐇𝐢-𝐑𝐞𝐬` → `Hi-Res`）。
- 搜索多条查询：完整清洗标题 + 前两个词（投稿人常在标题末尾附歌词片段，会大幅收窄结果）。
- 打分：标题相似度（编辑距离 + 子串包含 + **顺序无关的字符覆盖率**，以应对
  「晴天 周杰伦」与「周杰伦 晴天 2160P版」这类词序颠倒）× 0.75
  + **相对时长容差** × 0.2（B 站视频常有前奏/尾奏，绝对阈值会误杀）+ 上传者名 × 0.05。
- 依次尝试得分最高的前 3 个候选，跳过无歌词的（翻唱/伴奏）结果。

## 测试

### 离线单元测试（默认，CI 运行）

```bash
cargo test          # 90 个测试，不联网
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

覆盖 WBI 签名的黄金值、LRC 解析（多时间戳/offset/翻译合并）、标题清洗与匹配打分、
音质选择优先级、缓存校验与 LRU 淘汰、Cookie 域隔离、配置回退、平台路径等。

### 联网集成测试（需显式开启）

```bash
# 真实 API：WBI 签名被服务端接受、搜索、playurl、CDN 下载、歌词回退
cargo test --test live -- --ignored --nocapture

# 端到端：工作线程 → 下载 → 缓存 → 歌词，以及真实音频设备播放/跳转
cargo test --test live_worker -- --ignored --nocapture --test-threads=1
```

> `live_worker` 中的播放测试会打开系统音频设备；无声卡的环境会打印跳过而非失败。

### 去风险 spike

```bash
# 1) 能否解码 B 站 DASH fMP4/AAC-LC？自己下载样本，无需参数
cargo run --example spike_decode

# 2) 能否边下边播？（前缀下限、以及"追加无效"的证据）
#    不带参数时复用 spike_decode 下载到临时目录的样本
cargo run --example spike_streaming -- /tmp/listenbli-spike-137649199.m4s
```

`spike_decode` 验证「下载 B 站 DASH 音频 → Symphonia 解码」这一最关键假设，打印容器
盒子、解码时长、RMS/峰值，并导出 `listenbli-spike-preview.wav` 供试听；同时验证 seek。

`spike_streaming` 是流式播放设计依据的实验：扫描不同前缀长度能解出多少音频，并证明
「解码器打开后继续追加数据」是无效的（前缀 10.0s → 追加 5.99 MB 后仍是 10.0s）。
它也是判断"是否真的在流式"的可复现证据。

### 跨平台验证

```bash
rustup target add x86_64-pc-windows-msvc
cargo check --target x86_64-pc-windows-msvc --all-targets
```

因全部依赖均为纯 Rust（`schannel`/`windows-sys` 等无需 C 编译器），`cargo check`
可在 macOS 上完成 Windows 目标的类型级校验。**运行时行为（出声、扫码、字体渲染）
仍需在 Windows 上实测**，`.github/workflows/ci.yml` 已配置 macOS + Windows 矩阵。

## 项目结构

```
src/
├── main.rs           eframe 启动、CJK 字体注入、windows_subsystem
├── app.rs            应用状态、事件泵、面板布局
├── platform.rs       平台差异（目录/字体/权限）——唯一含 cfg 分支的模块
├── config.rs         设置与 cookie 持久化
├── net.rs            工作线程与 命令/事件 总线
├── util.rs           标题清洗、时长解析、相似度
├── api/              client / wbi / cookie / models / search / video / login / library
├── audio/            engine(rodio) / cache(磁盘缓存 + LRU)
├── lyrics/           lrc(解析) / bilibili(字幕) / netease(第三方回退)
└── ui/               theme(设计令牌) / icons(矢量图标) / widgets(控件)
                      mod(外壳: 顶栏/列表/状态栏) / player / lyrics / login
                      library(收藏夹+历史) / settings(设置面板)
```

线程模型：UI 线程（永不阻塞） + API 工作线程（解析、登录、库） + 封面线程 + 歌词线程
+ 下载线程（边下边播、带进度、按请求代次取消）。

封面和歌词各自独立成线程，是为了不让它们挡住点击：一次搜索会给每个结果要一张封面，
如果和「解析 + 取播放地址」共用一个队列，用户点歌时要先排在 20 张图片后面（实测 3.2s）。
拆开之后同一次点击的解析只用 0.09s。

## 已知限制

- 仅播放音频，不含视频画面。
- 不做音频下载/导出功能。
- 歌曲的发现方式为搜索 + 收藏夹 + 历史；未实现每日推荐/分区浏览。
- 网易云为第三方非官方接口，可能被限流或失效；届时歌词会优雅降级为「暂无歌词」。
- 收藏夹/历史接口的完整字段未经真实账号逐一核对，缺失字段会跳过而非崩溃。
- **开播后最初的几秒内**，向前拖到尚未缓冲的位置会被拒绝并提示"音频仍在缓冲"；
  缓冲完成后（本机约 4 秒、6 MB 的歌曲）任意位置均可跳转。这是为避免 UI 线程
  在解码器定位时阻塞而做的取舍。
- 缓存为整段保存而非分段稀疏下载，因此一首歌要么完整命中缓存，要么重新下载。

## 免责声明

本项目仅用于学习与技术研究，所有音频与歌词内容的版权归原作者及平台所有。
请勿用于任何商业用途或大规模抓取。
