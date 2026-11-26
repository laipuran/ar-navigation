// test.js —— 自测脚本：以子进程启动服务 + 逐个接口验证
// 运行：node test.js
// 全部通过输出 "ALL PASS"，任何失败以非零退出码结束（方便以后接 CI）。
//
// 【契约边界怎么测】对话接口的真上游（Rust 后端）在测试机上是缺席的，
// 所以这里起两个 BFF 实例 + 一个进程内 mock 上游：
//   BFF#1（18787）：上游指向一个肯定没人听的死端口 -> 测 502 MODEL_UNAVAILABLE 和参数校验
//   BFF#2（18789）：上游指向 mock（18788）        -> 测 200 透传 / 429 Retry-After / 504 超时
// mock 的行为由 message 内容决定：'ratelimit' 回 429，'slow' 慢 1.5 秒，其余回 200。

const assert = require('assert')
void assert
const { spawn } = require('child_process')
const http = require('http')

function waitForServer(url, timeoutMs) {
  return new Promise(function (resolve, reject) {
    const started = Date.now()
    function tryOnce() {
      fetch(url).then(function (r) { resolve(r) }).catch(function () {
        if (Date.now() - started > timeoutMs) return reject(new Error('服务启动超时'))
        setTimeout(tryOnce, 200)
      })
    }
    tryOnce()
  })
}

// 进程内 mock 上游：记录最后一次收到的请求体（用于验证 conversation_id 透传）
function startMockUpstream(port) {
  const lastBody = { value: null }
  const server = http.createServer(function (req, res) {
    let raw = ''
    req.on('data', function (c) { raw += c })
    req.on('end', function () {
      let body = {}
      try { body = JSON.parse(raw || '{}') } catch (e) {}
      lastBody.value = body
      if (body.message === 'ratelimit') {
        res.writeHead(429, { 'Content-Type': 'application/json; charset=utf-8', 'Retry-After': '7' })
        return res.end(JSON.stringify({ error: { code: 'RATE_LIMITED', message: '慢点问' } }))
      }
      if (body.message === 'slow') {
        // 故意慢 1.5 秒：配合 BFF#2 的 ASSISTANT_TIMEOUT_MS=400 触发 504
        return setTimeout(function () {
          res.writeHead(200, { 'Content-Type': 'application/json; charset=utf-8' })
          res.end(JSON.stringify({ conversation_id: 'c_slow', reply: '终于回上了' }))
        }, 1500)
      }
      res.writeHead(200, { 'Content-Type': 'application/json; charset=utf-8' })
      res.end(JSON.stringify({ conversation_id: 'c_test123', reply: '模拟回复：教四在南门对面' }))
    })
  })
  return new Promise(function (resolve) {
    server.listen(port, function () { resolve({ server, lastBody }) })
  })
}

function postJson(base, payload) {
  return fetch(base + '/api/v1/chat/messages', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: typeof payload === 'string' ? payload : JSON.stringify(payload)
  })
}

async function main() {
  const mock = await startMockUpstream(18788)

  // BFF#1：死上游（127.0.0.1:59999 基本没人监听 -> 连接被拒 -> 502）
  const proc1 = spawn(process.execPath, ['server.js'], {
    env: Object.assign({}, process.env, { PORT: '18787', ASSISTANT_BASE_URL: 'http://127.0.0.1:59999' }),
    stdio: ['ignore', 'pipe', 'pipe']
  })
  // BFF#2：mock 上游 + 极短超时（400ms，专门用来触发 504）
  const proc2 = spawn(process.execPath, ['server.js'], {
    env: Object.assign({}, process.env, {
      PORT: '18789',
      ASSISTANT_BASE_URL: 'http://127.0.0.1:18788',
      ASSISTANT_TIMEOUT_MS: '400'
    }),
    stdio: ['ignore', 'pipe', 'pipe']
  })

  const B1 = 'http://127.0.0.1:18787'
  const B2 = 'http://127.0.0.1:18789'
  let passed = 0
  let failed = 0

  function check(name, condition, detail) {
    if (condition) { passed++; console.log('  PASS  ' + name) }
    else { failed++; console.log('  FAIL  ' + name + (detail ? '  -> ' + detail : '')) }
  }

  try {
    await waitForServer(B1 + '/api/v1/health', 5000)
    await waitForServer(B2 + '/api/v1/health', 5000)
    console.log('两个 BFF 实例均已启动，开始测试...\n')

    // ---------- 1. 健康检查 ----------
    {
      const r = await fetch(B1 + '/api/v1/health')
      const j = await r.json()
      check('GET /health 200', r.status === 200)
      check('health 返回 X-Request-ID', Boolean(r.headers.get('x-request-id')))
      check('health 报告上游不可达（死端口）', j.assistantReachable === false)
    }
    {
      const r = await fetch(B2 + '/api/v1/health')
      const j = await r.json()
      check('health 报告上游可达（mock 在跑）', j.assistantReachable === true)
    }

    // ---------- 2. 楼栋列表 ----------
    {
      const r = await fetch(B1 + '/api/v1/buildings')
      const j = await r.json()
      check('GET /buildings 200', r.status === 200)
      check('4 栋楼', j.buildings.length === 4, String(j.buildings.length))
      check('教四已建图', j.buildings.find(b => b.id === 'j4').mapped === true)
    }

    // ---------- 3. 地点解析：各种写法 ----------
    {
      const r = await fetch(B1 + '/api/v1/places/resolve?name=' + encodeURIComponent('教四-302'))
      const j = await r.json()
      check('resolve 教四-302 ok', j.ok === true)
      check('楼栋 = 第四教学楼', j.name === '第四教学楼')
      check('楼层 = 3', j.floor === 3)
      check('室内图 = 有', j.hasIndoorMap === true)
      check('房间节点 = r302', j.roomNodeId === 'r302')
    }
    {
      const r = await fetch(B1 + '/api/v1/places/resolve?name=' + encodeURIComponent('教 四 － 305'))
      const j = await r.json()
      check('resolve 容错写法（空格+全角横线）', j.ok === true && j.room === '305')
    }
    {
      const r = await fetch(B1 + '/api/v1/places/resolve?name=' + encodeURIComponent('教五-101'))
      const j = await r.json()
      check('resolve 教五 ok 但无室内图', j.ok === true && j.hasIndoorMap === false && j.roomNodeId === null)
    }
    {
      const r = await fetch(B1 + '/api/v1/places/resolve?name=' + encodeURIComponent('明工教305'))
      const j = await r.json()
      check('resolve 未收录 -> 404 PLACE_NOT_FOUND', r.status === 404 && j.error.code === 'PLACE_NOT_FOUND')
    }

    // ---------- 4. 楼内算路 ----------
    {
      const r = await fetch(B1 + '/api/v1/routes/plan?name=' + encodeURIComponent('教四-302'))
      const j = await r.json()
      check('route 教四-302 ok', j.ok === true)
      // 5 + 8 + 12 + 12 + 8 + 5 = 50
      check('总路程 = 50 米', j.totalMeters === 50, String(j.totalMeters))
      check('第一步是进南门后的走廊', j.steps[0].type === 'walk')
      check('包含上楼步骤（到 3 楼）', j.steps.some(s => s.type === 'stairs' && s.toFloor === 3))
      check('末步是到达 302', j.steps[j.steps.length - 1].text === '到达 302')
      // 路径经过两个转角：j-lobby-1 直行；j-lobby-3 处来路 stairs-3 下标0、去路 r302 下标3，n=4，diff=3 -> 左转
      check('在 j-lobby-3 推断出左转', j.steps.some(s => s.turn === '左转'))
    }
    {
      // r304 路径：stairs-3 -> j-lobby-3(直行) -> j-corridor-3(右转) -> r304
      const r = await fetch(B1 + '/api/v1/routes/plan?name=' + encodeURIComponent('教四-304'))
      const j = await r.json()
      check('route 教四-304 出现右转', j.steps.some(s => s.turn === '右转'))
    }
    {
      const r = await fetch(B1 + '/api/v1/routes/plan?name=' + encodeURIComponent('教五-101'))
      const j = await r.json()
      check('route 未建图 -> 404 ROUTE_NOT_MAPPED', r.status === 404 && j.error.code === 'ROUTE_NOT_MAPPED')
    }
    {
      // 图中不存在 999 教室
      const r = await fetch(B1 + '/api/v1/routes/plan?name=' + encodeURIComponent('教四-999'))
      const j = await r.json()
      check('route 房间不存在 -> 404 ROOM_NOT_IN_GRAPH', r.status === 404 && j.error.code === 'ROOM_NOT_IN_GRAPH')
    }

    // ---------- 5. OCR（mock） ----------
    {
      const r = await fetch(B1 + '/api/v1/ocr/recognize', { method: 'POST' })
      const j = await r.json()
      check('ocr 返回 mock 引擎', j.engine === 'mock')
      check('ocr 6 门课', j.courses.length === 6)
    }

    // ---------- 6. 对话：参数校验（契约 api.md:18） ----------
    {
      const r = await postJson(B1, { message: '   ' })
      const j = await r.json()
      check('chat 空白消息 -> 400 INVALID_REQUEST', r.status === 400 && j.error.code === 'INVALID_REQUEST')
    }
    {
      const r = await postJson(B1, { message: '你好', extra: '多余字段' })
      const j = await r.json()
      check('chat 未知字段 -> 400 INVALID_REQUEST（不再静默丢弃）', r.status === 400 && j.error.code === 'INVALID_REQUEST')
    }
    {
      const r = await postJson(B1, { message: '你好', conversation_id: '' })
      const j = await r.json()
      check('chat 空 conversation_id -> 400（不再当成新对话）', r.status === 400 && j.error.code === 'INVALID_REQUEST')
    }
    {
      const r = await postJson(B1, { message: '你好', conversation_id: 'x'.repeat(129) })
      const j = await r.json()
      check('chat 超 128 字节 conversation_id -> 400', r.status === 400 && j.error.code === 'INVALID_REQUEST')
    }
    {
      const r = await postJson(B1, { message: '好'.repeat(4001) })
      const j = await r.json()
      check('chat message 超 4000 字 -> 400', r.status === 400 && j.error.code === 'INVALID_REQUEST')
    }
    {
      // 40KB 的请求体，必须收到 400 响应而不是连接错误
      const r = await postJson(B1, { message: '大'.repeat(40000) })
      const j = await r.json()
      check('chat 超 32KiB -> 400 INVALID_REQUEST（响应可达）', r.status === 400 && j.error.code === 'INVALID_REQUEST')
    }
    {
      const r = await postJson(B1, '{ 这是坏 JSON')
      check('chat 非法 JSON -> 400', r.status === 400)
    }
    {
      const r = await fetch(B1 + '/api/v1/not/exist')
      check('未知接口 -> 404', r.status === 404)
    }

    // ---------- 7. 对话：BFF#1 死上游 -> 502 MODEL_UNAVAILABLE（契约 api.md:85） ----------
    {
      const r = await postJson(B1, { message: '教四-302 在哪' })
      const j = await r.json()
      check('chat 上游不可达 -> 502 MODEL_UNAVAILABLE', r.status === 502 && j.error.code === 'MODEL_UNAVAILABLE')
    }

    // ---------- 8. 对话：BFF#2 + mock 上游，走通整条链路 ----------
    {
      const r = await postJson(B2, { message: '教四-302 在哪' })
      const j = await r.json()
      check('chat 成功 -> 200', r.status === 200)
      check('透传 conversation_id', j.conversation_id === 'c_test123')
      check('透传 reply', j.reply === '模拟回复：教四在南门对面')
      check('请求体只含 message（首轮不带 conversation_id）',
        mock.lastBody.value && mock.lastBody.value.message === '教四-302 在哪'
        && !('conversation_id' in mock.lastBody.value))
    }
    {
      // 续聊：conversation_id 必须原样透传给上游
      await postJson(B2, { message: '续聊一下', conversation_id: 'c_prev_abc' })
      check('续聊 conversation_id 原样透传',
        mock.lastBody.value && mock.lastBody.value.conversation_id === 'c_prev_abc')
    }
    {
      // 429 + Retry-After 必须一路透传到客户端（契约 api.md:84）
      const r = await postJson(B2, { message: 'ratelimit' })
      const j = await r.json()
      check('chat 429 RATE_LIMITED 透传', r.status === 429 && j.error.code === 'RATE_LIMITED')
      check('Retry-After 头透传（值 = 7）', r.headers.get('retry-after') === '7')
    }
    {
      // 上游慢 -> BFF 超时 -> 504 REQUEST_TIMEOUT（契约 api.md:86）
      const r = await postJson(B2, { message: 'slow' })
      const j = await r.json()
      check('chat 上游超时 -> 504 REQUEST_TIMEOUT', r.status === 504 && j.error.code === 'REQUEST_TIMEOUT')
    }

  } catch (e) {
    failed++
    console.error('测试执行异常:', e.message)
  } finally {
    proc1.kill()
    proc2.kill()
    mock.server.close()
  }

  console.log('\n结果: ' + passed + ' 通过 / ' + failed + ' 失败')
  if (failed > 0) process.exit(1)
  console.log('ALL PASS')
}

main()
