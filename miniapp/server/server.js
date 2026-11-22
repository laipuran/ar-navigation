// server.js —— miniapp 专属后端（BFF，Backend For Frontend）
//
// 【它在团队架构里的位置】
//
//   小程序(miniapp) ──> 本服务(Node) ──┬─ 地点解析/楼内算路/课表OCR：本地实现
//                                     └─ /chat/messages ──转发──> Rust 后端 /api/v1/assistant/messages
//
// 为什么不是把所有逻辑都塞进 Rust 后端？
//   1. 同学的 Rust 后端按 spec 只开放了对话 REST 接口，导航算路是进程内设计；
//   2. 本机没有 Rust 工具链，改 Rust 无法验证；
//   3. BFF 是"为前端定制的胶水层"，响应字段按小程序页面直接可用设计，
//      后端接口变化时只需要改这里，六个页面不用动。
//
// 【运行】
//   node server.js                    # 默认端口 8787
//   PORT=9000 node server.js          # 自定义端口
//   ASSISTANT_BASE_URL=http://127.0.0.1:8080 node server.js   # 指定 Rust 后端地址
//
// 【零依赖】只用 Node 内置模块，不需要 npm install。

const http = require('http')
const crypto = require('crypto')
const { URL } = require('url')

const { listBuildings } = require('./lib/buildings')
const { resolvePlace } = require('./lib/places')
const { planRoute } = require('./lib/routing')
const { recognize } = require('./lib/ocr')

const PORT = Number(process.env.PORT) || 8787
const ASSISTANT_BASE_URL = process.env.ASSISTANT_BASE_URL || 'http://127.0.0.1:8080'
const ASSISTANT_TIMEOUT_MS = Number(process.env.ASSISTANT_TIMEOUT_MS) || 60000
const BODY_LIMIT = 32 * 1024   // 与 Rust 后端保持一致的请求体上限

// ---------- 响应辅助 ----------

function sendJson(res, status, data, requestId) {
  const body = JSON.stringify(data)
  res.writeHead(status, {
    'Content-Type': 'application/json; charset=utf-8',
    'X-Request-ID': requestId,
    'Access-Control-Allow-Origin': '*'   // 方便浏览器调试；小程序不受 CORS 约束
  })
  res.end(body)
}

function sendError(res, status, code, message, requestId) {
  sendJson(res, status, { error: { code, message } }, requestId)
}

// 读取请求体（JSON）。三种结果：
//   对象        -> 正常
//   null        -> 超过 BODY_LIMIT（继续把数据读完再回 400，保证客户端能收到响应，
//                  而不是中途掐断连接让客户端只看到网络错误）
//   undefined   -> JSON 非法 / 读取出错
function readBody(req) {
  return new Promise(function (resolve) {
    let size = 0
    let tooLarge = false
    const chunks = []
    req.on('data', function (chunk) {
      size += chunk.length
      if (size > BODY_LIMIT) {
        tooLarge = true
        chunks.length = 0
        return    // 不再保存内容，但继续消费数据流，等 end 事件
      }
      chunks.push(chunk)
    })
    req.on('end', function () {
      if (tooLarge) return resolve(null)
      if (!chunks.length) return resolve({})
      try {
        resolve(JSON.parse(Buffer.concat(chunks).toString('utf8')))
      } catch (e) {
        resolve(undefined)         // JSON 非法
      }
    })
    req.on('error', function () { resolve(undefined) })
  })
}

// ---------- 业务处理器 ----------

const handlers = {

  // 健康检查
  'GET /api/v1/health': function (req, res, requestId) {
    sendJson(res, 200, {
      ok: true,
      service: 'miniapp-bff',
      assistantBackend: ASSISTANT_BASE_URL,
      uptimeSeconds: Math.round(process.uptime())
    }, requestId)
  },

  // 楼栋列表
  'GET /api/v1/buildings': function (req, res, requestId) {
    sendJson(res, 200, { ok: true, buildings: listBuildings() }, requestId)
  },

  // 黑话解析：GET /api/v1/places/resolve?name=教四-302
  'GET /api/v1/places/resolve': function (req, res, requestId, query) {
    const name = query.get('name') || ''
    const result = resolvePlace(name)
    if (!result.ok) {
      const code = result.reason === 'empty' ? 'EMPTY_INPUT' : 'PLACE_NOT_FOUND'
      const message = result.reason === 'empty'
        ? '请输入要查询的地点。'
        : '还没收录「' + (result.buildingPart || result.raw) + '」，请带上课表上的地点全称。'
      return sendError(res, 404, code, message, requestId)
    }
    sendJson(res, 200, result, requestId)
  },

  // 楼内路线：GET /api/v1/routes/plan?name=教四-302
  'GET /api/v1/routes/plan': function (req, res, requestId, query) {
    const name = query.get('name') || ''
    const place = resolvePlace(name)
    if (!place.ok) {
      return sendError(res, 404, 'PLACE_NOT_FOUND', '地点「' + name + '」无法解析。', requestId)
    }
    const result = planRoute(place)
    if (!result.ok) {
      return sendError(res, 404, result.code, result.message, requestId)
    }
    sendJson(res, 200, result, requestId)
  },

  // 课表识别（当前 mock）：POST /api/v1/ocr/recognize
  'POST /api/v1/ocr/recognize': function (req, res, requestId) {
    sendJson(res, 200, recognize(), requestId)
  },

  // 对话：转发到 Rust 后端 /api/v1/assistant/messages
  'POST /api/v1/chat/messages': async function (req, res, requestId) {
    const body = await readBody(req)
    if (body === null) {
      return sendError(res, 400, 'INVALID_REQUEST', '请求体超过 32 KiB 上限。', requestId)
    }
    if (body === undefined || typeof body !== 'object' || Array.isArray(body)) {
      return sendError(res, 400, 'INVALID_REQUEST', '请求体必须是 JSON 对象。', requestId)
    }
    // 契约：拒绝未知字段（api.md 第 18 行），只允许 message 和 conversation_id
    const known = ['message', 'conversation_id']
    for (const key of Object.keys(body)) {
      if (!known.includes(key)) {
        return sendError(res, 400, 'INVALID_REQUEST', '存在未知字段：' + key, requestId)
      }
    }
    if (typeof body.message !== 'string' || !body.message.trim()) {
      return sendError(res, 400, 'INVALID_REQUEST', '缺少非空 message 字段。', requestId)
    }
    // 契约：message 最多 4000 个 Unicode 字符
    if (Array.from(body.message).length > 4000) {
      return sendError(res, 400, 'INVALID_REQUEST', 'message 超过 4000 个字符上限。', requestId)
    }
    // 契约：conversation_id 不能是空字符串、最长 128 字节。
    // 空字符串是"非法续聊"，必须 400 拒绝，而不是悄悄当成新对话
    if ('conversation_id' in body) {
      const cid = body.conversation_id
      if (typeof cid !== 'string' || cid === '' || Buffer.byteLength(cid, 'utf8') > 128) {
        return sendError(res, 400, 'INVALID_REQUEST',
          'conversation_id 必须是 1~128 字节的字符串，开始新对话请省略该字段。', requestId)
      }
    }

    const payload = { message: body.message }
    // conversation_id 是会话凭据，原样透传，不做任何加工
    if (body.conversation_id) payload.conversation_id = body.conversation_id

    try {
      const upstream = await fetch(ASSISTANT_BASE_URL + '/api/v1/assistant/messages', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
        signal: AbortSignal.timeout(ASSISTANT_TIMEOUT_MS)
      })
      const text = await upstream.text()
      // 上游的错误结构（{error:{code,message}}）原样透传，前端处理逻辑不用区分来源
      const headers = {
        'Content-Type': 'application/json; charset=utf-8',
        'X-Request-ID': requestId
      }
      // 契约：429 RATE_LIMITED 必须带 Retry-After 头告诉客户端等多久（api.md 第 84 行）
      const retryAfter = upstream.headers.get('retry-after')
      if (retryAfter) headers['Retry-After'] = retryAfter
      res.writeHead(upstream.status, headers)
      res.end(text)
    } catch (e) {
      if (e && (e.name === 'TimeoutError' || e.name === 'AbortError')) {
        // 契约：等待上游超时 -> 504 REQUEST_TIMEOUT（api.md 第 86 行）
        sendError(res, 504, 'REQUEST_TIMEOUT', '对话请求超时，请原样重试。', requestId)
      } else {
        // 契约：上游不可达（Rust 后端没启动 / 网络问题）-> 502 MODEL_UNAVAILABLE
        sendError(res, 502, 'MODEL_UNAVAILABLE',
          '对话服务暂时不可用（' + ASSISTANT_BASE_URL + '），请稍后重试。', requestId)
      }
    }
  }
}

// ---------- 服务器主体 ----------

const server = http.createServer(async function (req, res) {
  const requestId = crypto.randomUUID()
  const started = Date.now()
  const url = new URL(req.url, 'http://localhost')

  // 预检请求直接放行（浏览器调试用）
  if (req.method === 'OPTIONS') {
    res.writeHead(204, {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type'
    })
    return res.end()
  }

  const key = req.method + ' ' + url.pathname
  const handler = handlers[key]

  try {
    if (!handler) {
      sendError(res, 404, 'NOT_FOUND', '接口不存在：' + key, requestId)
    } else {
      await handler(req, res, requestId, url.searchParams)
    }
  } catch (e) {
    console.error('[' + requestId + '] 处理失败:', e)
    if (!res.headersSent) {
      sendError(res, 500, 'INTERNAL_ERROR', '服务内部错误。', requestId)
    }
  }

  console.log('[' + requestId.slice(0, 8) + '] ' + req.method + ' ' + url.pathname +
    ' -> ' + res.statusCode + ' ' + (Date.now() - started) + 'ms')
})

server.listen(PORT, function () {
  console.log('miniapp BFF 已启动: http://127.0.0.1:' + PORT)
  console.log('  对话转发目标(Rust 后端): ' + ASSISTANT_BASE_URL)
  console.log('  接口清单:')
  for (const key of Object.keys(handlers)) console.log('    ' + key)
})
