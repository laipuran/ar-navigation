// test.js —— 自测脚本：以子进程启动服务（固定测试端口 18787）+ 逐个接口验证
// 运行：node test.js
// 全部通过输出 "ALL PASS"，任何失败以非零退出码结束（方便以后接 CI）。

const assert = require('assert')
void assert
const { spawn } = require('child_process')

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

async function main() {
  const proc = spawn(process.execPath, ['server.js'], {
    env: Object.assign({}, process.env, { PORT: '18787' }),
    stdio: ['ignore', 'pipe', 'pipe']
  })
  const BASE = 'http://127.0.0.1:18787'
  let passed = 0
  let failed = 0

  function check(name, condition, detail) {
    if (condition) { passed++; console.log('  PASS  ' + name) }
    else { failed++; console.log('  FAIL  ' + name + (detail ? '  -> ' + detail : '')) }
  }

  try {
    await waitForServer(BASE + '/api/v1/health', 5000)
    console.log('服务已启动（端口 18787），开始测试...\n')

    // 1. 健康检查
    {
      const r = await fetch(BASE + '/api/v1/health')
      const j = await r.json()
      check('GET /health 200', r.status === 200)
      check('health 返回 X-Request-ID', Boolean(r.headers.get('x-request-id')))
    }

    // 2. 楼栋列表
    {
      const r = await fetch(BASE + '/api/v1/buildings')
      const j = await r.json()
      check('GET /buildings 200', r.status === 200)
      check('4 栋楼', j.buildings.length === 4, String(j.buildings.length))
      check('教四已建图', j.buildings.find(b => b.id === 'j4').mapped === true)
    }

    // 3. 地点解析：各种写法
    {
      const r = await fetch(BASE + '/api/v1/places/resolve?name=' + encodeURIComponent('教四-302'))
      const j = await r.json()
      check('resolve 教四-302 ok', j.ok === true)
      check('楼栋 = 第四教学楼', j.name === '第四教学楼')
      check('楼层 = 3', j.floor === 3)
      check('室内图 = 有', j.hasIndoorMap === true)
      check('房间节点 = r302', j.roomNodeId === 'r302')
    }
    {
      const r = await fetch(BASE + '/api/v1/places/resolve?name=' + encodeURIComponent('教 四 － 305'))
      const j = await r.json()
      check('resolve 容错写法（空格+全角横线）', j.ok === true && j.room === '305')
    }
    {
      const r = await fetch(BASE + '/api/v1/places/resolve?name=' + encodeURIComponent('教五-101'))
      const j = await r.json()
      check('resolve 教五 ok 但无室内图', j.ok === true && j.hasIndoorMap === false && j.roomNodeId === null)
    }
    {
      const r = await fetch(BASE + '/api/v1/places/resolve?name=' + encodeURIComponent('明工教305'))
      const j = await r.json()
      check('resolve 未收录 -> 404 PLACE_NOT_FOUND', r.status === 404 && j.error.code === 'PLACE_NOT_FOUND')
    }

    // 4. 楼内算路
    {
      const r = await fetch(BASE + '/api/v1/routes/plan?name=' + encodeURIComponent('教四-302'))
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
      const r = await fetch(BASE + '/api/v1/routes/plan?name=' + encodeURIComponent('教四-304'))
      const j = await r.json()
      check('route 教四-304 出现右转', j.steps.some(s => s.turn === '右转'))
    }
    {
      const r = await fetch(BASE + '/api/v1/routes/plan?name=' + encodeURIComponent('教五-101'))
      const j = await r.json()
      check('route 未建图 -> 404 ROUTE_NOT_MAPPED', r.status === 404 && j.error.code === 'ROUTE_NOT_MAPPED')
    }
    {
      // 图中不存在 999 教室
      const r = await fetch(BASE + '/api/v1/routes/plan?name=' + encodeURIComponent('教四-999'))
      const j = await r.json()
      check('route 房间不存在 -> 404 ROOM_NOT_IN_GRAPH', r.status === 404 && j.error.code === 'ROOM_NOT_IN_GRAPH')
    }

    // 5. OCR（mock）
    {
      const r = await fetch(BASE + '/api/v1/ocr/recognize', { method: 'POST' })
      const j = await r.json()
      check('ocr 返回 mock 引擎', j.engine === 'mock')
      check('ocr 6 门课', j.courses.length === 6)
    }

    // 6. 对话转发（Rust 后端未启动 -> 502 UPSTREAM_UNAVAILABLE 属于预期）
    {
      const r = await fetch(BASE + '/api/v1/chat/messages', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ message: '教四-302 在哪' })
      })
      const j = await r.json()
      check('chat 上游不可用 -> 502', r.status === 502 && j.error.code === 'UPSTREAM_UNAVAILABLE')
    }

    // 7. 参数校验
    {
      const r = await fetch(BASE + '/api/v1/chat/messages', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ message: '   ' })
      })
      const j = await r.json()
      check('chat 空消息 -> 400', r.status === 400 && j.error.code === 'INVALID_REQUEST')
    }
    {
      const r = await fetch(BASE + '/api/v1/not/exist')
      check('未知接口 -> 404', r.status === 404)
    }

  } catch (e) {
    failed++
    console.error('测试执行异常:', e.message)
  } finally {
    proc.kill()
  }

  console.log('\n结果: ' + passed + ' 通过 / ' + failed + ' 失败')
  if (failed > 0) process.exit(1)
  console.log('ALL PASS')
}

main()
