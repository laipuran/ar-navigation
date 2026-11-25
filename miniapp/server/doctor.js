// doctor.js —— 链路体检：一条命令看清"谁在跑、谁没跑、断在哪"
//
// 用法：
//   node doctor.js
//   ASSISTANT_BASE_URL=http://192.168.1.7:3000 node doctor.js   # 体检同学的局域网后端
//
// 它同时检查两件事：
//   [1] BFF（本文件所在服务，默认 8787）有没有在跑、health 是否正常
//   [2] BFF 的上游 Rust 后端（默认 3000）能不能连上
// 最后给出结论：链路是否全通，以及哪一层功能会受影响。

const net = require('net')
const { URL } = require('url')

const PORT = Number(process.env.PORT) || 8787
const BFF_BASE = process.env.BFF_BASE || ('http://127.0.0.1:' + PORT)
const ASSISTANT_BASE_URL = process.env.ASSISTANT_BASE_URL || 'http://127.0.0.1:3000'

// TCP 探测：只看"这个端口有没有人监听"，不关心回什么内容
function tcpProbe(baseUrl, timeoutMs) {
  return new Promise(function (resolve) {
    let url
    try { url = new URL(baseUrl) } catch (e) {
      return resolve({ ok: false, host: '?', port: 0, reason: '地址格式非法：' + baseUrl })
    }
    const host = url.hostname
    const port = Number(url.port) || (url.protocol === 'https:' ? 443 : 80)
    const socket = net.connect({ host: host, port: port })
    let done = false
    function finish(ok, reason) {
      if (done) return
      done = true
      socket.destroy()
      resolve({ ok: ok, host: host, port: port, reason: reason || '' })
    }
    socket.setTimeout(timeoutMs)
    socket.on('connect', function () { finish(true) })
    socket.on('timeout', function () { finish(false, '超时（可能被防火墙拦截）') })
    socket.on('error', function (e) { finish(false, e.code === 'ECONNREFUSED' ? '端口没有服务在监听' : (e.code || e.message)) })
  })
}

async function httpGet(url, timeoutMs) {
  try {
    const r = await fetch(url, { signal: AbortSignal.timeout(timeoutMs) })
    return { ok: true, status: r.status, text: await r.text() }
  } catch (e) {
    return { ok: false, error: e.name || e.message }
  }
}

function line(text) { console.log(text) }

async function main() {
  line('')
  line('miniapp 链路体检')
  line('------------------------------------------------------------')
  line('BFF 地址      : ' + BFF_BASE)
  line('Rust 后端地址 : ' + ASSISTANT_BASE_URL)
  line('------------------------------------------------------------')

  let bffRunning = false
  let upstreamReachable = null

  // ---------- [1] BFF ----------
  const bffTcp = await tcpProbe(BFF_BASE, 800)
  if (!bffTcp.ok) {
    line('[1] BFF        : ✗ 未启动（' + bffTcp.reason + '）')
  } else {
    const h = await httpGet(BFF_BASE + '/api/v1/health', 2000)
    if (!h.ok) {
      line('[1] BFF        : △ 端口在监听，但 health 访问失败（' + h.error + '）')
    } else {
      bffRunning = true
      let info = null
      try { info = JSON.parse(h.text) } catch (e) {}
      line('[1] BFF        : ✓ 正常运行（进程已运行 ' + (info ? info.uptimeSeconds : '?') + ' 秒）')
      if (info && typeof info.assistantReachable === 'boolean') {
        upstreamReachable = info.assistantReachable
        line('     BFF 自己探测的上游 : ' + (info.assistantReachable ? '可达' : '不可达'))
      }
    }
  }

  // ---------- [2] Rust 后端（独立再测一次，不依赖 BFF 的结论） ----------
  const upTcp = await tcpProbe(ASSISTANT_BASE_URL, 800)
  if (!upTcp.ok) {
    line('[2] Rust 后端   : ✗ 不可达（' + upTcp.host + ':' + upTcp.port + ' ' + upTcp.reason + '）')
    upstreamReachable = false
  } else {
    // 端口在监听就说明服务活着；能访问到 Swagger 页面说明还是那个 Rust 后端
    const sw = await httpGet(ASSISTANT_BASE_URL + '/swagger-ui/', 2000)
    if (sw.ok) {
      line('[2] Rust 后端   : ✓ 可达（' + upTcp.host + ':' + upTcp.port + '，Swagger 返回 ' + sw.status + '）')
      upstreamReachable = true
    } else {
      line('[2] Rust 后端   : △ 端口在监听，但 HTTP 无响应（' + sw.error + '）')
      upstreamReachable = false
    }
  }

  // ---------- 结论 ----------
  line('------------------------------------------------------------')
  if (bffRunning && upstreamReachable) {
    line('结论：链路全通。对话页会走后端大模型（真链路）。')
    line('      若对话仍异常，看服务端日志里的 4xx/5xx，再对照 docs/specs/api.md 的错误码。')
  } else if (bffRunning && !upstreamReachable) {
    line('结论：断在【第 2 跳】——你的 BFF 正常，但够不到 Rust 后端。')
    line('      影响面：只有"对话页"少了大模型，会自动落到本地规则回复；')
    line('              地点解析 / 楼内算路 / 课表识别 全部正常（都是 BFF 自己实现的）。')
    line('      下一步：让同学启动后端并给你可达地址，然后：')
    line('              ASSISTANT_BASE_URL=http://<他的IP>:3000 node server.js')
  } else if (!bffRunning && upstreamReachable) {
    line('结论：Rust 后端在跑，但 BFF 没启动 —— 小程序只会看到请求失败。')
    line('      下一步：cd miniapp/server && node server.js')
  } else {
    line('结论：两个服务都没跑。先启动 BFF：')
    line('      cd miniapp/server && node server.js')
    line('      再把终端 A 挂着别关，另开一个窗口跑本脚本复检。')
  }
  line('')
}

main()
