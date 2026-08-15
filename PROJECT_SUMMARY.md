# 项目交接文档：拟物 iOS6 琥珀风格本地音乐播放器

> 本文件面向接手本项目的下一个 AI，完整描述：项目全貌、架构、数据流、已完成的每一项功能、
> 修复过的每一个 bug（含根因）、用户红线、构建/调试手册。请通读后再动手。

---

## 0. 快速上手（给下一个 AI）

| 项 | 值 |
|---|---|
| 项目根目录 | `D:\_workspace2\opencode\music-player` |
| 技术栈 | Tauri 2 (Rust) + Vue 3 (`<script setup>` + TS) + Vite 6 + AudioWorklet |
| 启动 | 双击 `start-dev.bat`（内部 `npm run tauri dev`，输出到 `dev.log` / `dev-err.log`） |
| 前端类型检查 | `npx vue-tsc --noEmit`（在项目根目录，须 exit 0） |
| Rust 编译 | 在 `src-tauri` 目录 `cargo build`（需先设环境变量，见 §9.1） |
| 运行日志 | `D:\_workspace2\opencode\music-player\dev.log`（stdout）、`dev-err.log`（stderr 含前端错误上报） |
| 数据目录 | `%APPDATA%\com.musicplayer.app\library.db`（SQLite，含曲库/播放状态/扫描目录） |
| 测试素材 | `D:\_Download\cloudmusic\`（test_wma.wma、test_flac.flac 带封面、test_flac.lrc）；用户真实曲库 `D:\_Logs\MP3`（约 1571 首，67 个 .lrc） |

---

## 1. 项目概述

一个 Windows 本地离线音乐播放器（Tauri 2 桌面应用），核心诉求与红线：

1. **视觉风格是用户的最强执念**：拟物（Skeuomorphic）iOS 6 琥珀色风格——深色底、琥珀金主色 `#e8a33d`、渐变、内阴影、高光、圆角按钮、发光小球。**任何改动不得破坏该风格**（用户曾多次因风格跑偏而否决）。
2. **支持全音频格式**：mp3/flac/wav/ogg/opus/m4a/aac/ape/wma/aiff/dsf 等（symphonia 主引擎 + ffmpeg 兜底）。
3. **界面元素学习主流开源播放器**（用户明确命令过）：网易云音乐（歌词详情页）、椒盐音乐 PC 版（布局）、YesPlayMusic / Feishin / Cider（进度条、音量位置等全屏布局惯例）。
4. **不得出现"说明性文字"**（如"歌词（点击跳转）"这类 UI 文案，用户明确否决）。
5. 用户无法提供截图，只能文字描述问题；用户对"修好一个又带坏一个"零容忍——**每次改动必须完整自检（vue-tsc + cargo build + 重启 + 日志无错）后才能交付**。

---

## 2. 目录结构

```
D:\_workspace2\opencode\music-player\
├── start-dev.bat            开发启动脚本（含全部编译环境变量，见 §9.1）
├── dev.log / dev-err.log    运行日志（dev-err 里有前端错误上报）
├── package.json             Vue/Vite/TS 依赖与脚本
├── vite.config.ts           端口 1420、vue 插件
├── tsconfig.json / tsconfig.node.json
├── index.html
├── app-icon.png
└── src\
    ├── main.ts              入口；全局 window.onerror + unhandledrejection → log_error 上报
    ├── App.vue              主界面：迷你模式 / 主窗口(标题栏+侧栏+内容+播放条) / 设置面板 / 详情页挂载
    ├── style.css            全局主题变量 + 拟物基础组件(.btn/.disc-btn/.card/.track-groove/.led/滚动条/分组列表)
    ├── player.ts            前端音频核心：AudioContext + Worklet 接入、play/togglePlay/seek/setVolume/进度保存/歌词加载/迷你窗口切换
    ├── player-processor.js  AudioWorkletProcessor：PCM 缓冲、重采样(线性+三次)、播放完成/缓冲 ack
    ├── types.ts             Song / LastState 接口
    ├── cover-cache.ts       Map<string,string> 封面 dataURL 缓存（模块级）
    ├── stores\player.ts     全局响应式 store（reactive）+ 队列/切歌/时间格式化
    └── components\
        ├── PlayerBar.vue    底部播放条：封面区(点击开详情页)+控制键+进度条+音量条
        ├── NowPlaying.vue   详情页全屏覆盖层（网易云式）：封面/控制/歌词(居中+点击跳转)/底部进度条/右下音量/左上收起/右上窗口三键
        ├── SongTable.vue    歌曲表格（序号+36px懒加载封面+标题/艺术家/专辑/时长，双击播放）
        ├── CoverImg.vue     封面组件：后端取图→缓存→懒加载(IntersectionObserver)→切歌动画
        ├── PIcon.vue        拟物 SVG 图标库（11 个图标，3 种色调，渐变+投影）
        └── LyricPanel.vue   ⚠️ 已弃用（旧常驻歌词栏，被 NowPlaying 取代，当前无引用，可删）
└── src-tauri\
    ├── tauri.conf.json      窗口 1200×800、decorations:false、bundle.resources=ffmpeg
    ├── capabilities\default.json  窗口权限清单（见 §5.4）
    ├── Cargo.toml           依赖清单
    ├── resources\           ffmpeg.exe / ffprobe.exe（9.0.1 gyan，各 ~100MB）+ 测试素材 + cover.png
    └── src\
        ├── main.rs          入口（Windows 无控制台）
        ├── lib.rs           全部 Tauri 命令 + 封面/歌词逻辑
        ├── player.rs        播放引擎：symphonia 主引擎 + ffmpeg 子进程引擎 + seek/pause/ack
        ├── db.rs            SQLite（rusqlite bundled）：songs/player_state/scan_dirs 三表
        └── music_lib.rs     曲库扫描：walkdir 递归 + lofty 读标签/时长，缺失字段回退文件名
```

---

## 3. 架构与数据流（核心知识）

### 3.1 播放流水线（最重要的架构）

```
[前端 player.ts]                          [后端 player.rs]                    [进程外]
  play(path)
    ├─ ensureCtx(): new AudioContext
    │   ├─ audioWorklet.addModule(player-processor.js)
    │   ├─ AudioWorkletNode("player-processor") + GainNode(音量)
    │   └─ node.port.onmessage: {type:"finished"}→自动下一曲; {type:"ack"}→invoke ack_chunks
    ├─ Channel 创建，ch.onmessage 把后端消息原样转发给 Worklet
    ├─ invoke("play_stream",{path,channel})
    └─ loadLyrics(path)

[后端 play_stream]
  区分格式：
  ├─ 18 种 symphonia 白名单扩展名 → open_stream() 线程解码
  │   └─ 每个音频 packet 解码为 f32 interleaved → channel.send(Audio)
  └─ 其他（ape/wma/dsf/tta 等）→ play_ffmpeg()
      ├─ ffprobe -of json 探测 sample_rate/duration → 先发 Header
      └─ ffmpeg -v error -i file -ac 2 -f f32le - 管道流式读 PCM

[前端 player-processor.js]
  - header 消息 → 记录 srcRate/channels
  - audio 消息 → append 到 Float32Array 缓冲
  - process() 每个渲染周期：按 srcRate/sampleRate 比例线性或三次插值重采样 → 输出
  - 缓冲不足 → 静音；buffered 数据>65536 帧 → 裁剪
  - end 消息 → 等缓冲耗尽 → 发 {type:"finished"}（前端据此自动下一曲）
  - 每累计 128 次 append → 发 ack（后端 PENDING 计数-1，用于背压）
```

**关键静态变量（player.rs）**：
- `GEN`（代际号）：每次 play/seek-ffmpeg 递增，解码线程/ffmpeg 循环检查自身代际是否过期 → 立即退出。用于防旧流覆盖新流。
- `PENDING`：已发送未 ack 的 chunk 数（上限 `MAX_PENDING=1024`），解码线程阻塞等待。
- `SENT_FRAMES` + `PLAY_START`：已发帧数 vs 播放墙钟，`ahead = SENT_FRAMES/rate - elapsed`，超过 `MAX_AHEAD=2s` 时解码线程 sleep 追平（供节流）。
- `PAUSED`：暂停标志（前端 suspend/resume 时同步）。
- `FFMPEG` / `CURRENT_PATH` / `CURRENT_CHANNEL`：ffmpeg 子进程句柄与当前流信息（seek 重启依赖）。

**seek 逻辑**：
- symphonia：`format.seek(Accurate, Time)`，随后重置 SENT_FRAMES = seconds*rate、PLAY_START 回拨、PENDING=0；前端同时 `baseTime = ctx.currentTime - sec` + 发 `{type:"clear"}` 清 Worklet 缓冲。
- ffmpeg：杀旧子进程 → GEN+1 → 用 `-ss {sec}` 重启 ffmpeg（ffmpeg 的 -ss 在 -i 前为快速 seek）。

**暂停逻辑**：前端 `ctx.suspend()/resume()` + 后端 `set_paused` 置位（并折算 SENT_FRAMES），后端解码线程每 20ms 轮询。

**自动下一曲**：Worklet 发 `finished` → `next()` 切到队列下一首 → `play(store.current.path)`（注意：曾修复过"finished 后播的是旧歌"的 bug，必须用 next() 之后的 store.current）。

### 3.2 封面链路

```
CoverImg.vue (props: path, size, lazy)
  ├─ load(): coverCache 命中→直接显示；未命中→invoke("get_cover",{path}) → base64 dataURL
  ├─ 非 lazy：mounted 即加载；lazy：IntersectionObserver(rootMargin 80px) 进入视口才加载
  ├─ watch(path)：切歌时立刻重新加载（修复"封面不更换"）
  ├─ <img :key="dataUrl"> + CSS cover-in 动画（每次封面变化淡入+缩放 0.92→1）
  └─ 无封面 → 琥珀渐变占位 + PIcon note 图标
后端 get_cover（lib.rs）：lofty 读内嵌图片，取 256B~8MB 的第一张，返回 data:image/...;base64
缓存：src/cover-cache.ts 模块级 Map
```

### 3.3 歌词链路

```
后端 get_lyrics(path)（lib.rs）：
  同名 .lrc（p.with_extension("lrc")）→ 读字节 → decode_lrc_bytes 多编码解码：
    UTF-8(剥 BOM/FEFF) → 否则 UTF-16LE → 否则 encoding_rs::GBK 兜底
  解析 [mm:ss.xx] 标签（支持一行多时间标签）→ 排序 → Vec<{time, text}>（time 单位=秒）
  日志：[lyrics] req ... / parsed N lines

前端 loadLyrics(path)（player.ts，export）：
  lyricFetchToken 防旧请求覆盖新结果；失败→空数组
  调用点：① play() 播放时 ② NowPlaying onMounted（若当前歌词为空，主动补拉——修复"恢复会话后没歌词"）

显示（NowPlaying.vue）：
  store.lyrics + store.lyricIndex（ticker 每 250ms 按 store.position 二分定位当前行）
  居中滚动：watch lyricIndex → scrollTo(el.offsetTop - panel.clientHeight/2 + el.clientHeight/2)，
    注意 np-lyric-inner 必须 position:relative（offsetTop 基准，否则不居中）
  点击歌词行 → seekTo(l.time)（跳到该行时间点）
  当前行：17px 加粗琥珀高亮+上浮+投影，上下区域 CSS mask 渐隐
  无歌词 → 显示"暂无歌词"（无 .lrc 属正常，用户曲库 1571 首仅 67 个 lrc）
```

### 3.4 持久化与状态恢复

```
保存：player.ts ticker 每 250ms 检查，距上次保存≥5s 才 invoke("save_progress",
      {songPath, position, queue(路径数组), volume})；play() 时 force 保存一次
恢复：App.vue onMounted 并行 invoke get_songs / get_scan_dirs / get_last_state
      → 恢复 volume（0~1 校验）、current/position/queue（按路径过滤回 Song 对象）
恢复不自动播放（只恢复界面状态，等用户点播放）
DB：player_state 单行表(id=1)；旧库自动 ALTER TABLE 补 volume 列（db.rs）
```

### 3.5 窗口模式（三种）

| 模式 | 实现 |
|---|---|
| 主窗口 | 1200×800，`decorations:false` 自绘标题栏；toolbar 有 `data-tauri-drag-region` + 双击最大化；右上 win-btns（迷你/最小化/最大化/关闭） |
| 迷你模式 | `toggleMini()`（player.ts）：记录原尺寸 → setResizable(false) → setSize(400×76) → setAlwaysOnTop(true)；退出反向。mini-app 模板整窗 `@pointerdown="miniDragStart"`，非 button 区域调 `win.startDragging()`（不要用 data-tauri-drag-region，它只对元素本身生效，导致拖不动——已修复） |
| 详情页 | NowPlaying.vue 全屏覆盖层（不是真全屏，用户把它当"全屏模式"）；左上「← 收起」返回；右上三键（最小化/最大化/关闭）；App.vue `<Transition name="np">` 缩放+淡入动画 |

**权限**（capabilities/default.json，缺一个窗口 API 就失效）：
`core:default`、`dialog:allow-open`、`core:window:allow-set-size/set-always-on-top/minimize/maximize/unmaximize/close/set-resizable/start-dragging/inner-size`

### 3.6 错误上报通道（调试利器）

- `main.ts` 全局 `window.onerror` + `unhandledrejection` → `invoke("log_error")` → 后端 `eprintln!("[webview-error] ...")` → 出现在 dev-err.log
- **接手后第一件事就是看 dev-err.log 有没有 `[webview-error]`**；曾靠它抓到"IntersectionObserver observe(null) 轰炸主线程导致全界面假死"这类隐蔽 bug
- 后端关键路径也有日志前缀：`[player]`（stream opened/sent chunks/ended）、`[lyrics]`、`[webview-error]`

---

## 4. 前端模块详解

### 4.1 stores/player.ts —— 全局 store

`reactive` 对象，字段（勿改名，多处引用）：
`songs`（全曲库）、`queue`（当前播放队列）、`current`（Song|null）、`currentIndex`、`playing`、`position`、`duration`、`volume`（默认 0.8）、`search`、`view`("all"|"album"|"artist")、`libraryDir`、`scanDirs`、`scanning`、`scrubbing`（拖动进度条中，ticker 暂停更新）、`mini`、`nowPlaying`（详情页开关）、`lyrics`、`lyricIndex`

函数：`setSongs`、`playAt`（未用）、`playQueue`（未用）、`next`/`prev`（环形队列取模）、`fmtTime`（m:ss）。

### 4.2 player.ts —— 前端音频核心

- 模块级单例：`ctx`(AudioContext)、`node`(AudioWorkletNode)、`gain`(GainNode)、`ticker`(setInterval 250ms)、`baseTime`、`playToken`(防旧 channel 覆盖新流)、`lastSave`、`origSize`(迷你模式原窗口尺寸)
- `toggleMini()`：迷你开关（见 §3.5），`getCurrentWindow` 动态 import（避免 build 分块警告可忽略）
- `ensureCtx()`：懒创建 AudioContext；Worklet onmessage 处理 finished（自动下一曲）/ack（背压）
- ticker：`store.position = max(0, ctx.currentTime - baseTime)` → `updateLyricIndex()` → `saveProgress()`
- `play(path)`：resume ctx → clearStream → baseTime=now → 建 Channel（`playToken` 递增，onmessage 里 token 不符直接丢弃，防旧流消息污染新流；header 的 duration>0 时写入 store.duration）→ `invoke("play_stream")` → saveProgress(true) → loadLyrics
- `togglePlay()`：ctx.suspend/resume + invoke set_paused（同步后端节流）
- `seekTo(sec)`：baseTime 回拨 + position 直接设 + 发 clear + invoke seek_stream
- `seekPreview(sec)`：拖动中只改 position（UI 预览）
- `setVolume(v)`：改 store + gain.gain
- `loadLyrics(path)`：export，token 防竞态

### 4.3 player-processor.js —— AudioWorkletProcessor

- `append`：Float32Array 拼接缓冲；每 128 次 append 发 ack
- `process`：按 ratio=srcRate/sampleRate 重采样；ratio≈1 直拷；否则三次(Catmull-Rom)插值；缓冲不足静音；`ended && 无缓冲` → 发 `finished` 并 return false（结束）
- 裁剪：readFrames>65536 时丢弃已消费部分
- `clear` 消息：清缓冲（seek 时用）

### 4.4 App.vue —— 主界面（767 行）

**模板结构**：
```
store.mini ? .mini-app(整窗拖动) : .app
  .toolbar（drag-region，双击最大化）
    .brand（logo+标题+播放LED）
    .toolbar-right：.search-box(高40px) + 齿轮按钮 + .win-btns(迷你/最小化/最大化/关闭)
  .body
    .sidebar（导航：全部歌曲/专辑/歌手 + 计数 + 底部库目录信息）
    .content（空态提示 / 专辑分组视图 / 歌手分组视图 / SongTable）
  PlayerBar
  设置弹层（showSettings，右上角面板：扫描目录列表+移除+添加+重扫）
  <Transition name="np"> NowPlaying（v-if store.nowPlaying，缩放+淡入 0.3s）
```

**script 要点**：
- `onMounted`：get_songs → get_scan_dirs（→scanDirs/libraryDir）→ get_last_state（恢复音量/当前歌/进度/队列）
- `addDir`/`removeDir`/`scanAll`：设置面板目录管理；scanAll 循环每个目录 invoke scan_library 后 setSongs 合并；removeDir 后端会级联删该目录曲库（db.rs `DELETE FROM songs WHERE path LIKE 'dir\%'`）
- `filtered`：search 对 title/artist/album 小写包含匹配
- `albumGroups`/`artistGroups`：Map 分组 → {name, songs}
- `switchView(v)`、`miniDragStart(e)`（非 button 才 startDragging）、`winMin/winToggleMax/winClose`
- `miniDragStart` 引用 `win`（getCurrentWindow），注意函数声明顺序（const win 在其后，函数体内才调用，无 TDZ 问题）

**注意**：模板 `<template v-else>` 外层闭合与 `<SongTable v-else>` 的嵌套层级曾出过错（见 §7 bug1/2），改动此区域务必整体核对标签配对。

### 4.5 PlayerBar.vue —— 底部播放条

- `pb-now`（封面56+标题/艺术家）整块点击 → `store.nowPlaying = true`（开详情页）
- 控制：disc-btn 上一首/播放暂停(main, amber)/下一首
- 进度：`pb-progress`（**touch-action:none 必须有**）+ track-groove.progress(flex:1, 高14px) + 时间两侧；拖动 = pointerdown/move/up 三件套，seekPreview 拖动中、seekUp 时 seekTo
- 音量：vol-track（16px 高，5 档步进 `Math.round(ratio*5)/5`，vol-fill/vol-steps/vol-thumb 拟物滑块）+ 百分比
- 样式均 scoped；track-groove/track-fill 全局在 style.css

### 4.6 NowPlaying.vue —— 详情页（446 行，网易云/主流播放器惯例布局）

**布局**（学习 YesPlayMusic/Feishin/Cider 后定稿）：
```
.np-overlay (fixed 全屏, z-100, 深色渐变)
  .np-back（左上「← 收起」胶囊按钮 → store.nowPlaying=false）
  .np-win（右上三键：最小化─/最大化□/关闭✕，关闭 hover 红色渐变）
  .np-stage（居中 flex）
    .np-left（220px 封面 + 标题 + 艺术家 + 控制三键(main 56px)）——无进度条无音量
    .np-right（歌词滚动区，高460px，无标题文字；上下 mask 渐隐；当前行放大高亮；
              每行可点击 → seekTo(l.time)；暂无歌词提示）
  .np-progress（absolute 底部横贯：左当前时间 + 进度条 flex:1 + 右总时长）
  .np-volume（absolute 右下角：音量图标 + 110px 滑块 + 百分比）
```

**script 要点**：
- `seekPct` 为 ref，watch store.position 更新（scrubbing 时不更新）；拖动三件套
- `scrollLyricTo(idx)`：居中滚动；watch lyricIndex + onMounted（打开时若有 index 立即居中一次）
- `onMounted`：若 current 存在且 lyrics 为空 → loadLyrics 主动补拉（恢复会话场景）
- 音量：volPct 按 5 档取整，volDown/volMove/volUp
- `winToggleMax`：isMaximized 判断切换；三键直接 win.minimize()/win.close()

### 4.7 SongTable.vue —— 歌曲表

- props: `songs: Song[]`；双击行 → 整表作队列 → play(song.path)
- 列：标题(序号+36px懒加载封面+标题文本+正在播放小三角play图标) / 艺术家 / 专辑 / 时长
- 正在播放行：琥珀高亮背景+标题加粗
- **传 lazy 封面**（几十到几百行时必须懒加载）；:key="song.path"

### 4.8 CoverImg.vue —— 封面组件（详见 §3.2）

关键：ref 同时挂在 v-if/v-else 两个分支（**lazy 时 mounted 阶段 v-if 分支不渲染，若 ref 只在 v-if 分支会 observe(null) 抛错炸主线程**——历史教训）；watch(path) 重载；img :key 触发 cover-in 动画。

### 4.9 PIcon.vue —— 拟物图标库

- 11 个图标：prev/next/play/pause/vol/search/note/mini/songs/album/artist
- tone：dark（深琥珀，用在 amber 主键上）/ light（白灰，普通）/ amber（琥珀金，强调）
- 每个实例生成唯一 `uid` 作渐变 id（gid=`pic-{uid}`、sid=`pic-{uid}-s`；**曾因 id 重复导致多个图标引用同一渐变而显示错乱**）
- `filter: drop-shadow(0 1px 1px rgba(0,0,0,.45))` 投影
- 默认尺寸表（不传 size 时）：prev/next/vol=15、play/pause=18、search=14、note=17、mini=13、songs/album/artist=15

### 4.10 style.css —— 全局主题

- 变量：--bg #121318 / --bg-2 #17181d / --card #1d1f26 / --card-2 #23262f / --accent #e8a33d / --accent-strong #f0b85a / --accent-soft rgba(232,163,61,.14) / --text #e6e8ee / --text-dim #8a8f9a / --radius 10px
- `.btn`（拟物：上高光+下渐变）、`.btn.primary`（琥珀渐变深字）、`.disc-btn`（圆形 radial 渐变球，`.main` 琥珀，`.small` 迷你用）、`.card`、`.track-groove/.track-fill`（全局进度条，含 18px 圆头+高光）、`.led`（状态灯）、滚动条、`.group-list/.group-item/.group-head`（专辑/歌手分组）
- `body { user-select:none; overflow:hidden }`

### 4.11 main.ts —— 入口 + 错误上报（见 §3.6）

---

## 5. 后端模块详解

### 5.1 lib.rs —— 命令注册表

`invoke_handler` 注册的全部命令：

| 命令 | 参数 → 返回 | 说明 |
|---|---|---|
| `greet` | name → String | 模板遗留 |
| `scan_library` | dir → Vec<Song> | 扫描目录并 upsert 入库 |
| `save_progress` | song_path, position, queue, volume | 存播放状态（4 参，含 volume） |
| `get_last_state` | → Option<LastState> | 恢复 |
| `get_songs` | → Vec<Song> | 从库读全曲库 |
| `get_cover` | path → Option<String> | lofty 读内嵌图 → data URL |
| `get_lyrics` | path → Vec<{time,text}> | 见 §3.3（含多编码解码） |
| `get_scan_dirs` / `add_scan_dir` / `remove_scan_dir` | → / path / path | 扫描目录管理 |
| `log_error` | message | 前端错误上报（eprintln 到 stderr） |
| `play_stream` | path, channel | 启动解码流 |
| `seek_stream` | seconds → Result | 两种引擎分支 seek |
| `ack_chunks` | n | 背压回执 |
| `set_paused` | paused | 暂停同步 |

`LyricLine { time: f64, text: String }`（time 秒）。`parse_lrc_time` 解析 `mm:ss(.xx)`。`decode_lrc_bytes` 编码兼容链。

### 5.2 player.rs —— 播放引擎（524 行，核心）

详见 §3.1。补充细节：
- `SYMPHONIA_EXTS` 白名单 18 种；`is_ffmpeg_format` = 不在白名单 → ffmpeg
- `find_ffmpeg` 查找顺序：`FFMPEG_DIR` 环境变量 → `CARGO_MANIFEST_DIR/resources`（dev 主路径）→ exe 旁 resources → PATH
- `probe_ffmpeg`：ffprobe json 取 sample_rate / duration
- `play_ffmpeg`：`-v error [-ss x] -i path -ac 2 -f f32le -`，stdout 管道读 65536 帧(2ch) 块转 f32 发送；结尾若代际未过期则 kill 子进程
- `set_paused`：折算 SENT_FRAMES 保持节流基准
- `open_stream`：symphonia probe → 默认轨 → decoder(verify:false) → 65536 帧 SampleBuffer；声道数不符时（首个包）重建 buffer 并重发 Header
- `decode_loop`：ResetRequired 容错（上限 64 次）；DecodeError 连续 200 次判 eos
- 未使用警告（无碍运行）：`CHUNK_FRAMES` 常量、`stream_info()` 函数

### 5.3 db.rs —— SQLite

- 三表：`songs`（path 主键，含 last_position/play_count 未用列）、`player_state`（单行 id=1）、`scan_dirs`
- 迁移：`ALTER TABLE player_state ADD COLUMN volume`（`.ok()` 忽略已存在）
- `upsert_songs`：事务 + ON CONFLICT 更新元数据
- `remove_scan_dir`：删目录 + 级联删 `songs WHERE path LIKE 'dir\%'`
- `get_last_state`：queue 为 JSON 数组字符串

### 5.4 music_lib.rs —— 扫描与标签

- `AUDIO_EXTS` 14 种（含 ape/wma/dsf）
- lofty 读 title/artist/album/track/year/genre/duration；title 空→文件名、artist 空→"未知艺术家"、album 空→"未知专辑"
- `scan_dir`：WalkDir 递归、排序（artist→album→track→title）

### 5.5 配置

- `tauri.conf.json`：窗口 1200×800/min 800×600、`decorations:false`、devUrl 1420、resources=ffmpeg.exe+ffprobe.exe、csp:null
- `capabilities/default.json`：见 §3.5 权限清单
- `Cargo.toml`：tauri 2、serde/serde_json、lofty 0.22、symphonia 0.5(all)、rusqlite 0.32(bundled)、walkdir、base64、tauri-plugin-dialog、**encoding_rs 0.8**（歌词 GBK 解码）

---

## 6. 已完成功能清单（按时间线，每条含实现位置）

1. **项目初始化**：Tauri 2 + Vue 3 + Vite + TS 脚手架，1200×800 窗口，深色主题全局变量
2. **主界面骨架**：toolbar（品牌区+搜索+窗口键）/ sidebar（导航+目录信息）/ content / PlayerBar
3. **曲库扫描入库**：music_lib.rs 14 种格式递归扫描 + db.rs upsert；启动从 DB 恢复曲库（无需重扫）
4. **播放引擎**：symphonia 解码 → Channel 推送 PCM → AudioWorklet 重采样输出；背压（ack/PENDING/MAX_AHEAD 节流）、暂停折算、symphonia seek
5. **ffmpeg 全格式支持**：下载 ffmpeg 9.0.1 gyan（ffmpeg.exe+ffprobe.exe ~100MB×2）放入 resources，tauri.conf 打包；ape/wma/dsf 等走 ffmpeg 管道；ffmpeg seek 杀进程重启；白名单分流
6. **拟物 iOS6 琥珀风格**：全局渐变/内阴影/高光体系（.btn/.disc-btn/.card/.track-groove/.led），用户确认风格
7. **PIcon 拟物图标库**：11 图标 3 色调，SVG 渐变+投影；uid 防 id 冲突
8. **播放条 PlayerBar**：封面区(点开详情页)/控制键/可拖进度条/5 档音量滑块
9. **歌曲列表 SongTable**：序号+懒加载封面+标题/艺术家/专辑/时长，双击整表入队播放，当前行高亮
10. **专辑/歌手视图**：侧栏导航切换，分组列表→展开组内歌曲（SongTable 复用）
11. **搜索**：顶部搜索框（40px 高，240px 宽），实时过滤标题/艺术家/专辑
12. **迷你模式**：400×76 置顶小窗，禁缩放，整窗可拖（非按钮区），上/下/播放控制+退出键
13. **自定义标题栏**：decorations:false，toolbar 拖拽+双击最大化，右上迷你/最小化/最大化/关闭四键
14. **封面全链路**：get_cover（lofty）→ cover-cache → CoverImg（lazy 懒加载 + 切歌 watch 重载 + 淡入缩放动画），各处尺寸：列表 36/播放条 56/迷你 52/详情页 220
15. **歌词全链路**：get_lyrics（多编码兼容 GBK/UTF-16/BOM）→ loadLyrics（token 防竞态）→ 详情页居中滚动+当前行放大+点击跳转
16. **详情页 NowPlaying**：网易云式（封面左+歌词右），后按主流开源播放器惯例改版：进度条横贯底部、音量右下角、左上收起、右上窗口三键、开合动画（Transition scale+fade）
17. **设置面板**：齿轮弹层，扫描目录列表（可单删）、添加文件夹（dialog）、重新扫描全部；"选择文件夹/重扫"按钮从工具栏移入设置
18. **多目录扫描**：scanDirs 表 + scanAll 循环合并
19. **状态恢复**：save_progress（5s 节流 + 切歌强制）记录歌/进度/队列/音量；启动恢复（不自动播放）
20. **音量记忆**：volume 列 + 恢复 + 保存
21. **错误上报通道**：window.onerror/unhandledrejection → log_error → dev-err.log
22. **自动下一曲**：Worklet finished → next() → play 新歌（修复旧歌 bug）

---

## 7. 关键 Bug 修复记录（教训，避免重蹈覆辙）

| # | 症状 | 根因 | 修复 |
|---|---|---|---|
| 1 | 应用直接报错白屏，vite `Unexpected }`（模板 274 行/CSS 578 行） | 编辑时残留"孤儿 CSS"（无选择器的 `font-family...}` 悬挂块） | 删除孤儿代码；**每次大编辑后跑 vue-tsc + 重启看日志** |
| 2 | "点什么都不动"（界面假死），无报错 | CoverImg lazy 模式 `ref` 绑在 v-if 分支 → mounted 时 el=null → `IntersectionObserver.observe(null)` 抛 TypeError；列表几百行每行都抛，异常轰炸主线程 | ref 移到 v-if/v-else 两个分支 + observe 前判空兜底 |
| 3 | 部分图标颜色/填充错乱 | PIcon 渐变 id 静态（`pic-{name}-{tone}`），多实例 id 冲突，fill 引用到第一个实例 | 每实例随机 uid 生成 id |
| 4 | 歌词读不到（显示"暂无歌词"） | lrc 为 GBK/ANSI 编码时 `read_to_string`（UTF-8）报错被 catch 吞掉；UTF-8 BOM 文件首行歌词丢失 | `decode_lrc_bytes`：UTF-8(剥BOM)→UTF-16→GBK 三级解码；`[lyrics]` 日志 |
| 5 | 恢复会话后打开详情页无歌词 | loadLyrics 只在 play() 调用；恢复状态不播放 → 歌词永远空 | NowPlaying onMounted 若歌词空且 current 存在 → 主动 loadLyrics |
| 6 | 详情页进度条又短又拖不动 | NowPlaying scoped 缺 `.track-groove.progress{flex:1}`（宽度塌缩≈0）+ 缺 `touch-action:none` | 补 flex:1 + touch-action；加宽至 320px；后按主流布局移到底部横贯 |
| 7 | 歌词不居中（当前行滚不到面板中央） | `.np-lyric-inner` 无 `position:relative`，offsetTop 相对 fixed overlay 计算错误 | inner 加 relative；onMounted 打开时主动居中一次 |
| 8 | 封面不随切歌更换（播放条/迷你/详情页） | CoverImg 常驻实例，只变 path prop，无 watch 重载 | watch(path) → disconnect io → 重新 load；img :key 触发淡入动画 |
| 9 | 迷你模式几乎拖不动 | `data-tauri-drag-region` 只对属性元素本身生效；实际点击都在子元素上 | 改整窗 `@pointerdown="miniDragStart"`：非 button 才 `win.startDragging()` |
| 10 | 自动下一曲播的是旧歌 | finished handler 里 `play(store.current.path)` 在 next() 前被调用（或用了旧引用） | 先 `next()` 再 `play(store.current.path)` |
| 11 | 编辑器残留 `.search-box input` 样式重复块+孤儿属性 | 分两次编辑同一选择器 | 一次性重写该选择器块，杜绝分段编辑 |

**通用教训**：① Vue 模板结构编辑（尤其 `</template>` 嵌套）后必须整体核对配对；② 所有 await/invoke 都要 catch；③ 大型重写用整文件 write，小改动用精确 edit；④ 每次交付前跑完整验证链（vue-tsc→cargo build→重启→grep 日志）。

---

## 8. 已知问题 / 待办（给下一个 AI）

- ⚠️ `LyricPanel.vue` 已弃用无引用，可删除（或保留参考）
- 未使用警告：player.rs `CHUNK_FRAMES` 常量、`stream_info()` 函数（可清理）
- 播放模式（列表循环/单曲循环/随机）未实现；目前 next/prev 为环形队列
- 桌面歌词（全局置顶歌词）未实现（用户提过"学习椒盐/网易云"，可能是未来需求）
- 音效（EQ/变调/降速）未实现（VutronMusic 有此功能，用户学习清单里提过）
- 歌曲内嵌歌词（非 .lrc 外挂）未支持；lrc 文件名必须与音频同名同目录
- `playAt`/`playQueue`（stores）未使用；songs 表 last_position/play_count 列未用
- 详情页/迷你模式与系统媒体键（SMTC/媒体控制）未集成
- ffmpeg 未放行时播放 ape/wma/dsf 会报"未找到 ffmpeg"错误消息（dev 模式 resources 路径正常，打包后靠 tauri.conf resources）
- 音量"5 档步进"是刻意设计（拟物滑块风格），改动前确认用户意图
- 用户曲库 `D:\_Logs\MP3`：1571 首但仅 67 个 .lrc；"暂无歌词"对无 lrc 的歌是**正常**行为，不要误判为 bug

---

## 9. 构建 / 运行 / 调试手册

### 9.1 环境变量（start-dev.bat 已固化，手动编译时需重设）

```
RUSTUP_HOME=D:\_workspace2\.rustup
CARGO_HOME=D:\_workspace2\.cargo
CARGO_TARGET_DIR=C:\rs-target
LIB=C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64;C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\ucrt\x64;D:\_workspace2\VS2022\BuildTools\VC\Tools\MSVC\14.44.35207\lib\x64
INCLUDE=...(同 start-dev.bat 第 6 行)
```

### 9.2 标准重启流程（改动后必须执行）

```powershell
# 1) 前端类型检查（项目根目录）
npx vue-tsc --noEmit            # 必须输出 tsc=0

# 2) 杀所有相关进程（exe 被占用会报 "failed to remove file"）
Get-CimInstance Win32_Process | ? { $_.Name -match "music-player|node|tauri" } | % { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
Get-CimInstance Win32_Process | ? { $_.Name -eq "cmd.exe" -and $_.CommandLine -match "start-dev" } | % { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
Start-Sleep -Seconds 3

# 3) Rust 编译（仅 Rust 改动时；src-tauri 目录下，先设 §9.1 环境变量）
cargo build     # 看 Finished 与 error

# 4) 重启（WMI 脱离进程树方式）
Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{ CommandLine = 'cmd.exe /c "D:\_workspace2\opencode\music-player\start-dev.bat"' }

# 5) 等待 ~40s（cargo 增量编译 + vite 启动），确认进程存活
Get-Process -Name "music-player*"   # 应有 PID

# 6) 检查日志无错
Select-String -Path "D:\_workspace2\opencode\music-player\dev-err.log" -Pattern "webview-error|panic|Pre-transform"  # 应无新条目
# dev.log 应有 VITE ready + Running C:\rs-target\debug\music-player.exe
```

### 9.3 调试技巧

- **前端运行时错误** → 看 dev-err.log 的 `[webview-error]`（需要用户操作触发后查看）
- **播放问题** → `[player]` 日志：stream opened / sent N chunks / stream ended / ffmpeg stream
- **歌词问题** → `[lyrics] req path=... lrc_exists=... parsed N lines`
- 想加后端诊断 → 直接 `eprintln!`（会进 dev-err.log），用完记得清理

### 9.4 测试素材

| 文件 | 用途 |
|---|---|
| `D:\_Download\cloudmusic\test_wma.wma` | ffmpeg 引擎验证（无封面） |
| `D:\_Download\cloudmusic\test_flac.flac` | 已注入 300×300 橙色封面 → 验证 get_cover |
| `D:\_Download\cloudmusic\test_flac.lrc` | 5 行测试歌词（UTF-8） |
| `src-tauri\resources\test.flac / test.wma / test_cover.flac` | 打包测试资源 |
| `D:\_Logs\MP3\alib\` | 用户真实曲库（含 GBK 编码日文 lrc，验证解码兼容） |

---

## 10. 用户沟通红线（重要）

1. **风格**：拟物 iOS6 琥珀（渐变/内阴影/高光/圆润）。用户对"看起来不像拟物"极度敏感。
2. **无说明性文字**：UI 上不要出现解释性文案（如"点击跳转"、"支持 xx 格式"可留，但界面标签别加括号说明）。
3. **布局学习对象**：网易云音乐（歌词详情页）、椒盐音乐 PC 版、YesPlayMusic/Feishin/Cider（全屏播放页：进度条底部横贯、音量右下角、左上返回、右上窗口键）。
4. **交付标准**：每次交付前完成 §9.2 全链路自检；不要"修好 A 弄坏 B"。
5. **交互预期**：详情页开合要有动画；封面切换要有动画；迷你窗口必须好拖；音量、进度条要 5 档拟物风格（用户偏好）。
6. 用户偏好"先问清楚再动手"（他多次强调），方向不确定时用 question 工具确认。
7. 测试与用户沟通时，请用户"操作后我再查日志"，不要空口断言已修复。