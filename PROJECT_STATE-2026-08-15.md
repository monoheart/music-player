# 项目当前状态文档（第一版完成 · 2026-08-15）

> 本文档反映 **2026-08-15 第一版完成时**的真实代码状态，供接手者快速掌握。
> 早期项目历程见保留的 `PROJECT_SUMMARY.md`（已过时，部分内容如 ffmpeg 引擎/play_count 已删除，以本文档为准）。

---

## 0. 快速上手

| 项 | 值 |
|---|---|
| 项目根目录 | `D:\_workspace2\opencode\music-player` |
| 技术栈 | Tauri 2 (Rust) + Vue 3 (`<script setup>` + TS) + Vite 6 + AudioWorklet |
| 启动 | 双击 `start-dev.bat`（内部 `npm run tauri dev`，日志到 `dev.log` / `dev-err.log`） |
| 前端类型检查 | `npx vue-tsc --noEmit`（项目根目录，须 exit 0） |
| Rust 编译 | 在 `src-tauri` 目录，先设环境变量（见 §7.1），`cargo build` |
| 数据目录 | `%APPDATA%\com.musicplayer.app\library.db`（SQLite：曲库/播放状态/扫描目录/歌单） |
| 安装包 | `src-tauri\target\release\bundle\nsis\music-player_0.1.0_x64-setup.exe`（NSIS，~2.2MB，实测可装可卸） |
| 版本控制 | git 已初始化（root-commit dee482f，master 分支） |

**音频支持**（symphonia 引擎，无 ffmpeg）：
`mp3 / mp2 / flac / wav / wave / ogg / oga / m4a / mp4 / aac / aiff / aif / pcm`

---

## 1. 已实现功能（第一版全量）

### 播放
- symphonia 解码 → Channel 推 PCM → AudioWorklet 重采样输出
- 背压（ack/PENDING/MAX_AHEAD 节流）、暂停/恢复 **200ms 淡入淡出**（worklet 状态机）
- seek（symphonia Accurate）+ 进度保存/恢复（5s 节流 + 切歌强制）
- 三种播放模式：**顺序 / 随机 / 单曲循环**（图标 loop/shuffle/repeat-one，模式持久化）
- 自动下一曲（worklet `finished` → `next()` → `play`）
- 世代（epoch）机制：每条流消息盖章，seek/切歌后旧数据自动作废

### 界面
- **主窗口**：自定义标题栏（拖拽/双击最大化/迷你/最小化/最大化/关闭）、侧栏（全部歌曲/专辑/歌手/收藏/歌单/设置）、顶部搜索、播放条
- **迷你模式**：400×76 置顶小窗，整窗可拖，上/下/播放/收藏/退出；**不响应任何全局快捷键**（空格等，避免与游戏冲突）
- **详情页**（网易云式）：封面+标题+控制键左侧、歌词滚动区右侧、底部横贯进度条、音量并入底部行、左上收起、右上窗口三键、文件详情面板、加入歌单、顶部拖拽区
- 窗口大小自适应：默认封面 380px，媒体查询多档（max-height 860/700、max-width 1100）保证不溢出；**最小窗口 1100×720**

### 曲库
- 多目录扫描（设置面板管理），symphonia 全格式
- lofty 读标签（标题/艺术家/专辑/时长/track/year/genre），缺失回退文件名
- 排序：自定义（拖拽）/ 标题 / 艺术家 / 专辑，方向升降；未拖过默认按标题拼音

### 歌单 / 收藏
- 歌单：创建/重命名/删除/排序/添加/移除，双击播放、右键菜单操作
- 收藏：全局统一 toggle（播放条、详情页、右键、迷你模式）
- 队列面板：查看/点击播放当前队列

### 封面 / 歌词
- 封面：lofty 取内嵌图 → dataURL → 模块级缓存 → 懒加载 + 切歌动画
- 歌词：.lrc 多编码解码（UTF-8/UTF-16/GBK），居中滚动、当前行放大高亮、点击跳转、行内多时间标签

### 其他
- 右键菜单：播放/下一首播放/添加到歌单/打开所在文件夹（歌单内另有"从歌单移除"）
- 「回到当前歌曲」按钮（列表视图中当前歌不在可视区时显示，点击平滑滚动居中）
- 文件详情（大小/格式/时长/采样率/比特率）
- 状态持久化：播放进度、队列、音量、播放模式、排序模式、窗口位置/大小
- 错误上报通道：`window.onerror/unhandledrejection` → `log_error` → dev-err.log `[webview-error]`

---

## 2. 架构与数据流

### 2.1 播放流水线

```
[前端 player.ts]                      [后端 player.rs]
  play(path)
    ├─ ensureCtx(): AudioContext + AudioWorklet 注册
    │   node.port.onmessage: finished→自动切歌 / ack→背压 / paused→真正挂起
    ├─ Channel 建连，ch.onmessage 原样转发给 Worklet（epoch 校验）
    ├─ invoke("play_stream",{path,channel})
    └─ loadLyrics(path)

[后端 player.rs]
  open_stream(): symphonia probe → 默认轨 → decoder → 65536 帧缓冲 → channel.send(Audio)
  decode_loop(): 背压(PENDING≤1024)/节流(MAX_AHEAD=2s)/ResetRequired 容错(≤64次)/epoch 检查
  seek_stream(): format.seek(Accurate, Time) + 重置节流基准；前端同时 baseTime 回拨 + clear 缓冲
  set_paused(): 折算 SENT_FRAMES 保持节流基准

[前端 player-processor.js]
  重采样（线性/三次 Catmull-Rom）、缓冲不足静音、end 后缓冲耗尽发 finished、
  pauseFading 状态机：0=正常 1=淡出 2=淡入 3=暂停静音（PAUSE_FADE=0.2s）
```

### 2.2 关键状态（player.rs 静态变量）
- `GEN`：代际号，play/seek 递增，旧流检查过期即退出
- `PENDING`：已发未 ack 块数（上限 1024），解码线程阻塞
- `SENT_FRAMES` + `PLAY_START`：节流基准（ahead > 2s 时 sleep 追平）
- `PAUSED`：暂停标志

### 2.3 数据库表（db.rs）
- `songs`（path 主键，title/artist/album/duration/format/track/year/genre/cover/last_position/updated_at/**sort_pos**/**fav**；旧库自动 `DROP COLUMN play_count`）
- `player_state`（单行 id=1：song_path/position/queue/volume/play_mode/window_state/sort_mode/sort_dir/custom_sort）
- `scan_dirs`、`playlists`、`playlist_songs`（级联删除，position 排序）
- 迁移方式：`ALTER TABLE ... ADD COLUMN`（`.ok()` 忽略已存在）+ `DROP COLUMN`

### 2.4 后端命令清单（lib.rs invoke_handler）
```
scan_library / save_progress / get_last_state / get_songs / get_cover / get_lyrics
get_scan_dirs / get_file_info / log_error / log_info / add_scan_dir / remove_scan_dir
play_stream / seek_stream / ack_chunks / set_paused / toggle_fav
create_playlist / rename_playlist / delete_playlist / list_playlists
add_to_playlist / remove_from_playlist / get_playlist_songs
reorder_playlists / add_songs_to_playlist / reorder_playlist_songs / reorder_songs
get_window_state / set_window_state / reveal_in_folder
```

### 2.5 窗口权限（capabilities/default.json）
`core:default` + `dialog:allow-open` + `core:window:allow-*`：
set-size / set-min-size / set-always-on-top / minimize / maximize / unmaximize / close / set-resizable / start-dragging / inner-size

---

## 3. 前端模块（src/）

| 文件 | 职责 |
|---|---|
| `main.ts` | 入口；全局 onerror/unhandledrejection → log_error |
| `App.vue` | 主界面骨架、状态恢复、快捷键、设置面板、右键菜单、迷你模板、队列面板、跳转按钮 |
| `player.ts` | 前端音频核心：ctx/worklet、play/togglePlay/seek/音量/迷你窗口/进度保存/歌词 |
| `player-processor.js` | AudioWorkletProcessor：PCM 缓冲、重采样、暂停淡变状态机、ack/finished |
| `playback.ts` | 播放模式图标/标签/循环、useSeek/useVolume、toggleFav、usePlaylistPicker |
| `types.ts` | Song / LastState / PlayMode / SortMode / Playlist 等接口 + SORT_OPTIONS |
| `style.css` | 拟物主题变量 + .btn/.disc-btn/.card/.track-groove/.led 等基础组件 |
| `cover-cache.ts` | 封面 dataURL 模块级缓存 |
| `stores/player.ts` | 全局 reactive store + next/prev/shuffle 顺序 + 歌单刷新 + toast |
| components/ | PlayerBar / NowPlaying / SongTable / CoverImg / PIcon / Marquee / QueuePanel |

---

## 4. 后端模块（src-tauri/src/）

| 文件 | 职责 |
|---|---|
| `lib.rs` | 全部 Tauri 命令（见 §2.4）+ 封面/歌词/窗口状态/文件信息 |
| `player.rs` | 播放引擎（symphonia）+ 3 个单元测试（decode_loop_sends_end_at_eof / real_mp3_seek_then_eof / seek_stream_increments_epoch） |
| `db.rs` | SQLite（rusqlite bundled）全部表操作 |
| `music_lib.rs` | 扫描（walkdir）+ lofty 标签 + `file_info()` 文件详情 |
| `main.rs` | 入口（Windows 无控制台） |

---

## 5. 关键修复记录（近期，避免重蹈覆辙）

| 症状 | 根因 | 修复 |
|---|---|---|
| 播完不自动切歌（偶发） | worklet 尾缓冲排空死锁竞态 | epoch 盖章 + 重排等待顺序；15 次真机 e2e 通过 |
| seek 后旧流卡死 | 解码循环 seek 后阻塞不结束 | 探针定位为竞态 + 世代机制作废旧数据 |
| 最小窗口详情页封面跑左上角 | 歌词 flex:1 吃满宽度 + 上下留白不均 | 歌词限宽 500px + stage justify-content:center + 顶部留白加大 |
| mini 模式在游戏中被空格暂停 | 全局 keydown 监听 Space | `onKeydown` 开头 `if (store.mini) return;`（本次修复） |
| 详情页小窗口 UI 溢出 | 单档媒体查询不够 | 三档 media query（max-height 860/700、max-width 1100）+ 窗口最小 1100×720 |
| 比特率显示 0 kbps | lofty bitrate 单位 kbps + 前端重复 /1000 | 去掉二次除法 + 体积估算兜底 |
| PowerShell 改含中文文件乱码 | Get-Content/Set-Content GBK 往返 | **只用 edit/write 工具改中文文件** |

---

## 6. 已知限制 / 待办

- **独占全屏游戏会遮挡 mini 窗口**：Windows 独占全屏（DXGI）是系统级行为，`setAlwaysOnTop` 无效；仅"无边框/窗口化全屏"时 mini 能显示。**已知限制，接受现状**
- 桌面歌词（全局置顶）未实现
- 音效（EQ/变调/降速）未实现
- 内嵌歌词（非 .lrc 外挂）未支持
- 系统媒体键（SMTC/媒体控制）未集成
- 用户曲库 `D:\_Logs\MP3`：1589 首，仅少数 .lrc；"暂无歌词"属正常
- 音量是 10 档步进（拟物风格设计，改动前确认意图）

---

## 7. 构建 / 运行 / 调试手册

### 7.1 环境变量（start-dev.bat 已固化，手动编译时需重设）
```
RUSTUP_HOME=D:\_workspace2\.rustup
CARGO_HOME=D:\_workspace2\.cargo
CARGO_TARGET_DIR 必须清掉（opencode 守护进程会继承旧的 C:\rs-target，跑 cargo 前 Remove-Item Env:CARGO_TARGET_DIR）
LIB=...Windows Kits 10.0.26100.0 um\x64;ucrt\x64;...MSVC 14.44.35207 lib\x64
INCLUDE=...(同 start-dev.bat)
```
> ⚠️ 构建产物 target 已钉死在项目内 `src-tauri\target`，禁止再指向 C 盘。

### 7.2 标准验证链（改动后必须执行）
```
1) npx vue-tsc --noEmit                        # 项目根目录，须 exit 0
2) cargo test                                   # src-tauri 目录，3 个测试须过
3) 杀进程链（node/cargo/music-player/rustc 按命令行匹配 music-player|tauri|vite|cargo）
4) 重启（start-app.vbs 或 start-dev.bat），等 ~40s
5) 检查 dev-err.log：无 [webview-error]/panic/Pre-transform
```

### 7.3 打包安装包
```
# src-tauri 目录（空间注意：D 盘仅剩 ~0.8GB，target/release 占 1.4GB）
npx tauri build --bundles nsis     # 生成 release/bundle/nsis/music-player_0.1.0_x64-setup.exe
```
- 实测流程（本次已完成）：静默安装 `/S` → 启动安装版 → CDP 验证（曲库/空格快捷键/mini）→ 卸载 `/S` 恢复注册表
- 安装版与 dev 共用 `%APPDATA%\com.musicplayer.app` 数据目录

### 7.4 调试技巧
- 前端错误 → dev-err.log 的 `[webview-error]`
- 播放 → `[player]` / `[audio]` / `[js]` 日志
- 歌词 → `[lyrics]`
- CDP 调试：启动前设会话级 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`，用 `C:\Users\MONOHE~1\AppData\Local\Temp\opencode\cdp-*.js` 脚本驱动
- 窗口缩放验证：`window.__TAURI_INTERNALS__.invoke('plugin:window|set_size',{value:{Logical:{width,height}}})`

---

## 8. 用户红线（不可违背）

1. **拟物 iOS6 琥珀风格**（`#e8a33d`、渐变/内阴影/高光/圆润）是最强执念，任何改动不得破坏
2. 界面不得出现"说明性文字"（如"点击跳转"这类解释文案）
3. 每次交付前完成 §7.2 全链路自检，不能"修好 A 弄坏 B"
4. 只能删除本会话创建的文件，不动用户电脑其他文件（`src-tauri/resources/ffmpeg.exe`、`ffprobe.exe` 是用户文件，不得删，也未打包）
5. 测试用 CDP 实测或"用户操作后查日志"，不要空口断言
6. 不确定方向时先问清楚再动手