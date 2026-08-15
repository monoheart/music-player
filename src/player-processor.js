/* AudioWorklet 处理器：接收后端 PCM 流。
   采样率相同 → 逐样本直通（位精确，零处理）；
   采样率不同 → Blackman-Harris 窗 sinc 高质量重采样（近透明）。
   全程 f32；seek/切歌时旧数据淡出到零再切换、新数据淡入，避免硬切爆音。 */
const TAPS = 128; // sinc 内核长度
const KERNEL_STEPS = 512; // 每周期内分数相位查表步数
const HALF = TAPS / 2;
const FADE_OUT = 240; // 淡出时长（输出帧，≈5.4ms @44.1k）
const FADE_IN = 480; // 淡入时长（输出帧，≈10.9ms @44.1k）
const PAUSE_FADE = 0.2; // 暂停/恢复淡变时长（秒）：小小淡入淡出，避免硬切爆音

class PlayerProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.buf = new Float32Array(0);
    this.readFrames = 0;
    this.srcRate = 44100;
    this.channels = 2;
    this.ended = false;
    this.finishedSent = false;
    this.appendCnt = 0;
    this.kernel = null;
    this.kernelRatio = 0;
    // 流世代：由后端在每条消息上盖章（play/seek 后递增）。
    // 新世代消息到达 → 此前缓冲的数据全部作废；旧世代残留块一律丢弃
    this.seq = 0;
    this.gotNewEpoch = false;
    this.fading = 0; // 0=正常 1=淡出 2=淡入（fadePos>FADE_IN 表示等待新数据，尚未开始淡入）
    this.fadePos = 0;
    this.wipeAt = 0; // clear 时刻缓冲的样本数：淡出完成后仅丢弃这部分，保留之后到达的新数据
    // 暂停/恢复淡变：暂停先淡出到零再通知主线程 suspend，恢复从零淡入，避免硬切爆音
    this.pauseFading = 0; // 0=正常 1=淡出中 2=淡入中 3=已暂停（静音，等待恢复）
    this.pauseFadeLeft = 0;
    this.pauseFadeTotal = Math.max(1, Math.round(sampleRate * PAUSE_FADE));
    this.port.onmessage = (e) => {
      const m = e.data;
      if (m.type === "pause") {
        // 淡入中再暂停：从当前增益对称翻转，无跳变；其余状态开始淡出
        if (this.pauseFading === 2) {
          this.pauseFading = 1;
          this.pauseFadeLeft = this.pauseFadeTotal - this.pauseFadeLeft;
        } else if (this.pauseFading === 0) {
          this.pauseFading = 1;
          this.pauseFadeLeft = this.pauseFadeTotal;
        }
        return;
      }
      if (m.type === "resume") {
        if (this.pauseFading === 1) {
          // 淡出中恢复：对称翻转，从当前增益平滑淡回
          this.pauseFading = 2;
          this.pauseFadeLeft = this.pauseFadeTotal - this.pauseFadeLeft;
        } else if (this.pauseFading === 3) {
          // 已暂停：从零开始淡入
          this.pauseFading = 2;
          this.pauseFadeLeft = this.pauseFadeTotal;
        }
        return;
      }
      if (m.type === "clear") {
        // 旧数据先淡出（期间继续渲染缓冲），到零后再切换，避免硬切爆音
        this.wipeAt = this.buf.length;
        this.fading = 1;
        this.fadePos = FADE_OUT;
        this.gotNewEpoch = false;
        return;
      }
      if (m.epoch !== undefined && m.epoch !== this.seq) {
        if (m.epoch < this.seq) return; // 旧世代残留（seek/切歌前的数据块），丢弃
        // 新世代：此前的数据全部作废
        this.seq = m.epoch;
        this.gotNewEpoch = true;
        if (this.fading === 1) {
          // 淡出进行中：把作废点推迟到淡出完成，旧音频自然淡到零后一次性丢弃
          this.wipeAt = this.buf.length;
        } else {
          this.buf = new Float32Array(0);
          this.readFrames = 0;
          this.ended = false;
          this.finishedSent = false;
          this.appendCnt = 0;
          if (this.fading === 2) this.fadePos = FADE_IN; // 等待中的数据已就绪，开始淡入
        }
      }
      if (m.type === "header") {
        this.srcRate = m.sample_rate;
        this.channels = m.channels;
        this.kernel = null; // 采样率变化 → 重建内核
      } else if (m.type === "audio") {
        // 等待新数据期间：只有确认收到新世代后才开始淡入（头消息先到、音频随后到的场景）
        if (this.fading === 2 && this.fadePos > FADE_IN && this.gotNewEpoch) this.fadePos = FADE_IN;
        this.append(new Float32Array(m.data));
      } else if (m.type === "end") {
        this.ended = true;
      }
    };
  }

  append(data) {
    if (data.length === 0) return;
    const nb = new Float32Array(this.buf.length + data.length);
    nb.set(this.buf);
    nb.set(data, this.buf.length);
    this.buf = nb;
    this.appendCnt++;
    if (this.appendCnt >= 128) {
      this.port.postMessage({ type: "ack", n: this.appendCnt });
      this.appendCnt = 0;
    }
  }

  at(frame, c) {
    const idx = frame * this.channels + c;
    if (idx < 0 || idx >= this.buf.length) return 0;
    return this.buf[idx];
  }

  // 量子内第 f 帧的增益（线性斜坡），淡出/淡入时随帧序号变化
  gainAt(f) {
    let g;
    if (this.fading === 1) g = (this.fadePos - f) / FADE_OUT;
    else if (this.fading === 2) g = this.fadePos > FADE_IN ? 0 : 1 - (this.fadePos - f) / FADE_IN;
    else return 1;
    return g < 0 ? 0 : g > 1 ? 1 : g;
  }

  // 暂停/恢复淡变增益（与 seek/切歌淡变叠加）
  pauseGainAt(f) {
    if (this.pauseFading === 1) {
      let g = (this.pauseFadeLeft - f) / this.pauseFadeTotal;
      return g < 0 ? 0 : g > 1 ? 1 : g;
    }
    if (this.pauseFading === 2) {
      let g = 1 - (this.pauseFadeLeft - f) / this.pauseFadeTotal;
      return g < 0 ? 0 : g > 1 ? 1 : g;
    }
    if (this.pauseFading === 3) return 0;
    return 1;
  }

  totalGain(f) {
    return this.gainAt(f) * this.pauseGainAt(f);
  }

  // 每量子结束后推进淡变状态机
  advanceFade(outFrames) {
    if (this.fading === 1) {
      this.fadePos -= outFrames;
      if (this.fadePos <= 0) {
        // 淡出完成：丢弃作废点之前的老数据，等待新世代（或确认已收到新世代）后淡入。
        // 缓冲里可能还有 clear 后到达的旧世代残留，一律不用于启动淡入
        this.buf = this.buf.slice(this.wipeAt);
        this.readFrames = 0;
        this.ended = false;
        this.finishedSent = false;
        this.appendCnt = 0;
        this.fading = 2;
        this.fadePos = this.gotNewEpoch ? FADE_IN : FADE_IN + 1;
        this.gotNewEpoch = false;
      }
    } else if (this.fading === 2 && this.fadePos <= FADE_IN) {
      this.fadePos -= outFrames;
      if (this.fadePos <= 0) this.fading = 0;
    }
    if (this.pauseFading === 1 || this.pauseFading === 2) {
      this.pauseFadeLeft -= outFrames;
      if (this.pauseFadeLeft <= 0) {
        if (this.pauseFading === 1) {
          // 淡出完成：保持静音，通知主线程真正 suspend
          this.pauseFading = 3;
          this.pauseFadeLeft = 0;
          this.port.postMessage({ type: "paused" });
        } else {
          this.pauseFading = 0;
          this.pauseFadeLeft = 0;
        }
      }
    }
  }

  // 按 ratio 构建 sinc 内核：相位取负 + 窗与 sinc 中心对齐，逐相位归一化保证直流增益=1
  buildKernel(ratio) {
    const cutoff = Math.min(1, 1 / ratio);
    const a0 = 0.35875, a1 = 0.48829, a2 = 0.14128, a3 = 0.01168;
    const k = new Float32Array(KERNEL_STEPS * TAPS);
    for (let q = 0; q < KERNEL_STEPS; q++) {
      const f = q / KERNEL_STEPS;
      const row = q * TAPS;
      let sum = 0;
      for (let i = 0; i < TAPS; i++) {
        // 输出样本位于 i1+frac，第 i 个抽头对应输入样本 i1+i-HALF，
        // 正确系数为 h(frac+HALF-i) = h(i-HALF-frac)（h 为偶函数）→ 相位取负。
        // 相位取正会得到镜像相位：输出位置变为 i1-frac，在输入区间内来回锯齿，严重失真
        const x = i - HALF - f;
        const px = Math.PI * cutoff * x;
        const s = x === 0 ? 1 : Math.sin(px) / px;
        // 窗与 sinc 共用同一自变量（以 x=0 为对称轴），保证内核严格偶对称
        const u = (x + HALF) / (2 * HALF);
        const w = a0 - a1 * Math.cos(2 * Math.PI * u) + a2 * Math.cos(4 * Math.PI * u) - a3 * Math.cos(6 * Math.PI * u);
        const v = s * w * cutoff;
        k[row + i] = v;
        sum += v;
      }
      if (sum !== 0) for (let i = 0; i < TAPS; i++) k[row + i] /= sum;
    }
    this.kernel = k;
    this.kernelRatio = ratio;
  }

  process(_inputs, outputs) {
    const out = outputs[0];
    const outFrames = out.length ? out[0].length : 0;
    if (outFrames === 0) return true;

    const ratio = this.srcRate / sampleRate;
    const needFrames = Math.ceil(outFrames * ratio) + TAPS;
    const availFrames = Math.floor(this.buf.length / this.channels) - this.readFrames;

    if (availFrames < needFrames) {
      // 数据不足：渲染剩余尾巴（<一量子），其余补零。
      // 必须把读位置推进到缓冲末尾——否则 availFrames 会停在 (0, needFrames) 区间不再
      // 变化，finished 永不触发，歌曲播完永远切不了下一首（历史根因）
      const tail = availFrames > 0 ? Math.floor(availFrames) : 0;
      const base = Math.floor(this.readFrames) * this.channels;
      for (let c = 0; c < out.length; c++) {
        const cc = c < this.channels ? c : this.channels - 1;
        const o = out[c];
        for (let f = 0; f < outFrames; f++) {
          let v = 0;
          if (f < tail) v = this.buf[base + f * this.channels + cc] ?? 0;
          o[f] = v * this.totalGain(f);
        }
      }
      if (this.ended && availFrames <= 0 && !this.finishedSent) {
        this.finishedSent = true;
        this.port.postMessage({ type: "finished" });
      }
      this.readFrames = Math.floor(this.buf.length / this.channels);
      this.advanceFade(outFrames);
      return true;
    }

    if (Math.abs(ratio - 1) < 1e-9) {
      // 直通：位精确，零插值；立体声走快速路径（无逐样本函数调用/越界检查）
      const base = Math.floor(this.readFrames) * this.channels;
      if (this.channels === 2 && out.length === 2) {
        const src = this.buf;
        const o0 = out[0];
        const o1 = out[1];
        if (this.fading === 0 && this.pauseFading === 0) {
          for (let f = 0; f < outFrames; f++) {
            const i = base + f * 2;
            o0[f] = src[i];
            o1[f] = src[i + 1];
          }
        } else {
          for (let f = 0; f < outFrames; f++) {
            const i = base + f * 2;
            const g = this.totalGain(f);
            o0[f] = src[i] * g;
            o1[f] = src[i + 1] * g;
          }
        }
      } else {
        for (let c = 0; c < out.length; c++) {
          const cc = c < this.channels ? c : this.channels - 1;
          const o = out[c];
          for (let f = 0; f < outFrames; f++) {
            const v = this.buf[base + f * this.channels + cc];
            o[f] = v * this.totalGain(f);
          }
        }
      }
      this.readFrames += outFrames;
    } else {
      if (!this.kernel || this.kernelRatio !== ratio) this.buildKernel(ratio);
      const k = this.kernel;
      for (let f = 0; f < outFrames; f++) {
        const srcPos = this.readFrames + f * ratio;
        const i1 = Math.floor(srcPos);
        const frac = srcPos - i1;
        // 相邻相位行线性插值，消除相位量化台阶（查表 512 步 + 行间 lerp）
        let q = frac * KERNEL_STEPS;
        if (q >= KERNEL_STEPS) q = KERNEL_STEPS - 1;
        const q0 = q | 0;
        const q1 = q0 + 1 < KERNEL_STEPS ? q0 + 1 : q0;
        const t = q - q0;
        const r0 = q0 * TAPS;
        const r1 = q1 * TAPS;
        const base = i1 - HALF;
        const g = this.totalGain(f);
        for (let c = 0; c < out.length; c++) {
          const cc = c < this.channels ? c : this.channels - 1;
          let acc = 0;
          for (let i = 0; i < TAPS; i++) acc += ((1 - t) * k[r0 + i] + t * k[r1 + i]) * this.at(base + i, cc);
          out[c][f] = acc * g;
        }
      }
      this.readFrames += outFrames * ratio;
    }

    this.advanceFade(outFrames);

    if (this.readFrames > 65536) {
      // 裁剪时保留 TAPS 帧历史（delay line）：重采样窗需要读取当前位置之前的样本，
      // 直接丢光会在每个缓冲边界截断窗函数，产生周期性伪影
      const consumed = Math.floor(this.readFrames) - TAPS;
      this.buf = this.buf.slice(consumed * this.channels);
      this.readFrames -= consumed;
    }
    return true;
  }
}

registerProcessor("player-processor", PlayerProcessor);
