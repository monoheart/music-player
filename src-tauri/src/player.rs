use serde::Serialize;
use std::fs::File;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration as StdDuration, Instant};
use symphonia::core::audio::{Channels, SampleBuffer, SignalSpec};
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SErr;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time as STime;
use tauri::ipc::Channel;

const MAX_PENDING: u64 = 1024;
const MAX_AHEAD: f64 = 2.0;

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamMsg {
    Header { epoch: u64, sample_rate: u32, channels: u16, duration: f64 },
    Audio { epoch: u64, data: Vec<f32> },
    End { epoch: u64 },
    Error { epoch: u64, message: String },
}

pub struct StreamState {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_buf: SampleBuffer<f32>,
    sample_rate: u32,
    channels: u16,
}

pub static STREAM: Mutex<Option<StreamState>> = Mutex::new(None);
pub static GEN: AtomicU64 = AtomicU64::new(0);
// 流世代：play/seek 时递增，发送的每条消息都盖章；worklet 只接受不小于当前世代的块，
// 新世代消息到达即作废旧数据。所有盖章在锁内/线程启动时完成，保证新世代块必然是新位置的数据
pub static EPOCH: AtomicU64 = AtomicU64::new(0);
pub static PENDING: Mutex<u64> = Mutex::new(0);
pub static SENT_FRAMES: AtomicU64 = AtomicU64::new(0);
pub static PAUSED: AtomicBool = AtomicBool::new(false);
pub static PLAY_START: Mutex<Option<Instant>> = Mutex::new(None);
pub static CURRENT_PATH: Mutex<Option<String>> = Mutex::new(None);
pub static CURRENT_CHANNEL: Mutex<Option<Channel<StreamMsg>>> = Mutex::new(None);

pub fn ack_chunks(n: u64) {
    let mut p = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    *p = p.saturating_sub(n);
}

pub fn set_paused(paused: bool) {
    PAUSED.store(paused, Ordering::SeqCst);
    if paused {
        let rate = {
            let guard = STREAM.lock().unwrap_or_else(|p| p.into_inner());
            guard.as_ref().map(|s| s.sample_rate as f64).unwrap_or(44100.0)
        };
        let elapsed = PLAY_START
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        SENT_FRAMES.store((elapsed * rate) as u64, Ordering::SeqCst);
    }
}

pub fn play_stream(path: String, channel: Channel<StreamMsg>) {
    *CURRENT_PATH.lock().unwrap_or_else(|p| p.into_inner()) = Some(path.clone());
    *CURRENT_CHANNEL.lock().unwrap_or_else(|p| p.into_inner()) = Some(channel.clone());
    EPOCH.fetch_add(1, Ordering::SeqCst);
    let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    *PLAY_START.lock().unwrap_or_else(|p| p.into_inner()) = Some(Instant::now());
    SENT_FRAMES.store(0, Ordering::SeqCst);
    PAUSED.store(false, Ordering::SeqCst);
    *PENDING.lock().unwrap_or_else(|p| p.into_inner()) = 0;
    std::thread::spawn(move || {
        if GEN.load(Ordering::SeqCst) != gen {
            return;
        }
        if let Err(e) = open_stream(&path, &channel) {
            let _ = channel.send(StreamMsg::Error {
                epoch: EPOCH.load(Ordering::SeqCst),
                message: e.to_string(),
            });
            *STREAM.lock().unwrap_or_else(|p| p.into_inner()) = None;
            return;
        }
        let rate = {
            let guard = STREAM.lock().unwrap_or_else(|p| p.into_inner());
            guard.as_ref().map(|s| s.sample_rate as f64).unwrap_or(44100.0)
        };
        decode_loop(&channel, gen, rate);
    });
}

fn open_stream(path: &str, channel: &Channel<StreamMsg>) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let ext = path.rsplit('.').next().unwrap_or("").to_string();
    let mut hint = Hint::new();
    if !ext.is_empty() {
        hint.with_extension(&ext);
    }
    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let format = probed.format;
    let track = format
        .default_track()
        .ok_or("no default audio track")?
        .clone();
    let track_id = track.id;
    let decoder = symphonia::default::get_codecs().make(
        &track.codec_params,
        &DecoderOptions { verify: false, ..Default::default() },
    )?;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels_mask =
        track.codec_params.channels.unwrap_or(Channels::FRONT_LEFT | Channels::FRONT_RIGHT);
    let channels = channels_mask.count() as u16;
    let spec = SignalSpec::new(sample_rate, channels_mask);
    let sample_buf = SampleBuffer::<f32>::new(65536, spec);
    *STREAM.lock().unwrap_or_else(|p| p.into_inner()) = Some(StreamState {
        format,
        decoder,
        track_id,
        sample_buf,
        sample_rate,
        channels,
    });
    let _ = channel.send(StreamMsg::Header {
        epoch: EPOCH.load(Ordering::SeqCst),
        sample_rate,
        channels,
        duration: track
            .codec_params
            .n_frames
            .map(|n| n as f64 / sample_rate as f64)
            .unwrap_or(0.0),
    });
    eprintln!("[player] stream opened: rate={sample_rate} ch={channels} path={path}");
    Ok(())
}

fn decode_loop(channel: &Channel<StreamMsg>, gen: u64, rate: f64) {
    let mut eos = false;
    let mut silence = 0usize;
    let mut sent = 0u64;
    let mut resets = 0u64;
    while !eos {
        if GEN.load(Ordering::SeqCst) != gen {
            return;
        }
        if PAUSED.load(Ordering::SeqCst) {
            std::thread::sleep(StdDuration::from_millis(20));
            continue;
        }
        let ahead = SENT_FRAMES.load(Ordering::SeqCst) as f64 / rate
            - PLAY_START
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .map(|t| t.elapsed().as_secs_f64())
                .unwrap_or(0.0);
        if ahead > MAX_AHEAD {
            std::thread::sleep(StdDuration::from_secs_f64(ahead - MAX_AHEAD));
        }
        loop {
            let pending = *PENDING.lock().unwrap_or_else(|p| p.into_inner());
            if pending < MAX_PENDING {
                break;
            }
            if GEN.load(Ordering::SeqCst) != gen {
                return;
            }
            std::thread::sleep(StdDuration::from_millis(10));
        }
        let mut guard = STREAM.lock().unwrap_or_else(|p| p.into_inner());
        let Some(st) = guard.as_mut() else {
            return;
        };
        let packet = match st.format.next_packet() {
            Ok(p) => p,
            Err(SErr::IoError(_)) => break,
            Err(SErr::ResetRequired) => {
                resets += 1;
                if resets > 64 {
                    eprintln!("[player] too many resets, aborting");
                    break;
                }
                continue;
            }
            Err(_) => break,
        };
        if packet.track_id() != st.track_id {
            continue;
        }
        match st.decoder.decode(&packet) {
            Ok(decoded) => {
                let frames = decoded.frames() as u64;
                let dch = decoded.spec().channels.count();
                if sent == 0 && dch as u16 != st.channels {
                    eprintln!("[player] channels mismatch: mask={} actual={dch}", st.channels);
                    st.sample_buf = SampleBuffer::<f32>::new(65536, decoded.spec().clone());
                    st.channels = dch as u16;
                    let _ = channel.send(StreamMsg::Header {
                        epoch: EPOCH.load(Ordering::SeqCst),
                        sample_rate: st.sample_rate,
                        channels: dch as u16,
                        duration: 0.0,
                    });
                }
                st.sample_buf.copy_interleaved_ref(decoded);
                let samples = st.sample_buf.samples().to_vec();
                silence = 0;
                // send 在锁内完成：与 seek_stream 的"seek+世代递增"严格互斥，
                // 保证世代递增后发出的数据块必然是 seek 之后的新位置
                if channel
                    .send(StreamMsg::Audio {
                        epoch: EPOCH.load(Ordering::SeqCst),
                        data: samples,
                    })
                    .is_err()
                {
                    return;
                }
                SENT_FRAMES.fetch_add(frames, Ordering::SeqCst);
                *PENDING.lock().unwrap_or_else(|p| p.into_inner()) += 1;
                sent += 1;
                if sent % 100 == 0 {
                    eprintln!("[player] sent {sent} chunks");
                }
            }
            Err(SErr::DecodeError(_)) => {
                silence += 1;
                if silence > 200 {
                    eos = true;
                }
            }
            Err(_) => break,
        }
    }
    *STREAM.lock().unwrap_or_else(|p| p.into_inner()) = None;
    let _ = channel.send(StreamMsg::End {
        epoch: EPOCH.load(Ordering::SeqCst),
    });
    eprintln!("[player] stream ended, chunks={sent} resets={resets}");
}

pub fn seek_stream(seconds: f64) -> Result<(), String> {
    let mut guard = STREAM.lock().unwrap_or_else(|p| p.into_inner());
    let Some(st) = guard.as_mut() else {
        return Ok(());
    };
    *PENDING.lock().unwrap_or_else(|p| p.into_inner()) = 0;
    let seek_to = SeekTo::Time {
        time: STime::new(seconds as u64, seconds.fract()),
        track_id: Some(st.track_id),
    };
    let rate = st.sample_rate as f64;
    let result = st
        .format
        .seek(SeekMode::Accurate, seek_to)
        .map(|_| ())
        .map_err(|e| e.to_string());
    if result.is_ok() {
        SENT_FRAMES.store((seconds * rate) as u64, Ordering::SeqCst);
        *PENDING.lock().unwrap_or_else(|p| p.into_inner()) = 0;
        *PLAY_START.lock().unwrap_or_else(|p| p.into_inner()) =
            Some(Instant::now() - StdDuration::from_secs_f64(seconds));
        eprintln!("[player] symphonia seek to {seconds:.2} ok");
        // 新世代：seek 成功后，解码循环后续发出的数据块必然是 seek 后的新位置
        EPOCH.fetch_add(1, Ordering::SeqCst);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;
    use tauri::ipc::InvokeResponseBody;

    // 测试共享 GEN/STREAM/PENDING 等全局量，必须串行执行
    static TEST_LOCK: StdMutex<()> = StdMutex::new(());

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// 生成 0.3s 44100Hz 单声道 16bit PCM wav，返回路径
    fn gen_wav(rate: u32, secs: u32) -> std::path::PathBuf {
        let frames = rate * secs;
        let path = std::env::temp_dir().join(format!("mp_eof_test_{rate}_{secs}.wav"));
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + frames * 2u32).to_le_bytes());
        b.extend_from_slice(b"WAVE");
        b.extend_from_slice(b"fmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * 2u32).to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(frames * 2u32).to_le_bytes());
        for i in 0..frames {
            let v = ((i as f64 * 440.0 * 2.0 * std::f64::consts::PI / rate as f64).sin()
                * 0.3
                * 32767.0) as i16;
            b.extend_from_slice(&v.to_le_bytes());
        }
        std::fs::write(&path, &b).unwrap();
        path
    }

    fn capture_channel() -> (Channel<StreamMsg>, std::sync::Arc<StdMutex<Vec<serde_json::Value>>>) {
        let received: std::sync::Arc<StdMutex<Vec<serde_json::Value>>> = std::sync::Arc::new(StdMutex::new(Vec::new()));
        let rx = received.clone();
        let ch = Channel::new(move |body| {
            let bytes: Vec<u8> = match body {
                InvokeResponseBody::Raw(b) => b,
                InvokeResponseBody::Json(s) => s.into_bytes(),
            };
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                rx.lock().unwrap().push(v);
            }
            Ok(())
        });
        (ch, received)
    }

    /// 整曲解码到 EOF 后必须发出 End，且每条消息都盖有 epoch
    #[test]
    fn decode_loop_sends_end_at_eof() {
        let _guard = test_lock();
        let path = gen_wav(44100, 1);
        let (ch, received) = capture_channel();
        let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
        open_stream(path.to_str().unwrap(), &ch).expect("open_stream");
        let rate = STREAM
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.sample_rate as f64)
            .unwrap();
        decode_loop(&ch, gen, rate);
        *STREAM.lock().unwrap() = None;

        let msgs = received.lock().unwrap().clone();
        assert!(!msgs.is_empty(), "no messages");
        let last = msgs.last().unwrap();
        assert_eq!(last["type"].as_str(), Some("end"), "expected End last");
        let audio = msgs.iter().filter(|m| m["type"].as_str() == Some("audio")).count();
        assert!(audio > 1, "expected multiple audio chunks, got {audio}");
        for m in &msgs {
            assert!(m.get("epoch").is_some(), "missing epoch stamp: {m}");
        }
        let _ = std::fs::remove_file(&path);
    }

    /// 真实 mp3 上：seek 到接近结尾后继续解码，必须能到 EOF 发出 End（真实环境曾卡死）
    #[test]
    fn real_mp3_seek_then_eof() {
        let _guard = test_lock();
        let dirs = [
            "D:\\_Logs\\MP3\\alib\\Pink Floyd - Stop.mp3",
            "D:\\_Logs\\MP3\\alib\\Oana - Otto A Totland.mp3",
        ];
        let Some(path) = dirs.iter().find(|p| std::path::Path::new(p).is_file()) else {
            eprintln!("[test] 找不到真实 mp3，跳过");
            return;
        };
        let (ch, received) = capture_channel();
        let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
        open_stream(path, &ch).expect("open_stream mp3");
        let rate = STREAM.lock().unwrap().as_ref().map(|s| s.sample_rate as f64).unwrap();
        let total = STREAM
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| {
                s.format
                    .tracks()
                    .first()
                    .and_then(|t| t.codec_params.n_frames)
                    .map(|n| n as f64 / rate)
                    .unwrap_or(0.0)
            })
            .unwrap_or(0.0);
        assert!(total > 10.0, "mp3 时长异常 {total}");
        // seek 到接近结尾，再跑真实解码循环 → 必须发出 End
        seek_stream(total - 2.0).expect("seek");
        decode_loop(&ch, gen, rate);
        *STREAM.lock().unwrap() = None;
        let msgs = received.lock().unwrap().clone();
        assert_eq!(msgs.last().unwrap()["type"].as_str(), Some("end"), "mp3 seek 后未以 End 收尾");
        let audio = msgs.iter().filter(|m| m["type"].as_str() == Some("audio")).count();
        assert!(audio > 0, "mp3 seek 后未发出任何 audio 块");
        let seek_epoch = msgs[0]["epoch"].as_u64().unwrap() + 1;
        assert!(msgs.iter().all(|m| m["epoch"].as_u64().unwrap() >= seek_epoch - 1), "epoch 异常");
        eprintln!("[test] mp3 ok: seek→{:.1}s→EOF，audio={audio} 块", total - 2.0);
    }

    /// seek 成功后 EPOCH 递增（新世代），此后发送的数据块带新章
    #[test]
    fn seek_stream_increments_epoch() {
        let _guard = test_lock();
        let path = gen_wav(44100, 2);
        let (ch, received) = capture_channel();
        let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
        open_stream(path.to_str().unwrap(), &ch).expect("open_stream");
        let before = EPOCH.load(Ordering::SeqCst);
        seek_stream(0.5).expect("seek");
        assert_eq!(EPOCH.load(Ordering::SeqCst), before + 1, "epoch must increment on seek");
        // seek 后继续解码到 EOF：所有后续消息的 epoch 均为新值
        let rate = STREAM.lock().unwrap().as_ref().map(|s| s.sample_rate as f64).unwrap();
        decode_loop(&ch, gen, rate);
        *STREAM.lock().unwrap() = None;
        let msgs = received.lock().unwrap().clone();
        let after_msgs: Vec<_> = msgs.iter().filter(|m| m["epoch"].as_u64() == Some(before + 1)).collect();
        assert!(!after_msgs.is_empty(), "no messages with the new epoch");
        for m in &msgs {
            let e = m["epoch"].as_u64().unwrap();
            assert!(e == before || e == before + 1, "unexpected epoch {e}");
        }
        let _ = std::fs::remove_file(&path);
    }
}
