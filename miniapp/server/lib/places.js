// lib/places.js —— 地点"黑话"解析
//
// 输入 "教四-302" / "教四302" / "教 四 － 302" 都能解析。
// 解析逻辑与小程序端 data/places.js 保持一致，
// 但数据源换成了 data/buildings.json（与 Rust 后端共用同一份）。
//
// 返回结构与小程序 result 页期望的字段完全对齐（titleText/floorText/...），
// 页面拿到即可渲染，不用做任何适配。

const { findBuilding, getGraph, getNodeMap } = require('./buildings')

// 归一化：去空格（含全角），统一各种横线为 "-"
function normalize(text) {
  return (text || '')
    .replace(/[\s\u3000]/g, '')
    .replace(/[－—–_]/g, '-')
}

// 房间号推楼层：302 -> 3，1201 -> 12，002 -> 1
function floorOf(room) {
  if (!room || room.length < 3) return null
  const num = parseInt(room.slice(0, room.length - 2), 10)
  if (isNaN(num)) return null
  return num === 0 ? 1 : num
}

// 在楼内图中找房间对应的节点（按 kind.name 匹配房间号）
function findRoomNode(buildingId, room) {
  const graph = getGraph(buildingId)
  if (!graph || !room) return null
  const node = graph.nodes.find(function (n) {
    return n.kind.type === 'room' && n.kind.name === room
  })
  return node ? node.id : null
}

// 核心：把一句"黑话"解析成结构化信息
function resolvePlace(input) {
  const raw = (input || '').trim()
  const key = normalize(raw)

  if (!key) {
    return { ok: false, raw, reason: 'empty' }
  }

  // 拆 "楼宇 + 房间号"：^(.+?)-?(\d{3,4})$
  const matched = key.match(/^(.+?)-?(\d{3,4})$/)
  let buildingPart = key
  let room = ''
  if (matched) {
    buildingPart = matched[1]
    room = matched[2]
  }

  const building = findBuilding(buildingPart)
  if (!building) {
    return { ok: false, raw, reason: 'not_found', buildingPart, room }
  }

  const floor = floorOf(room)
  const graph = getGraph(building.id)
  const roomNodeId = findRoomNode(building.id, room)

  return {
    ok: true,
    raw,
    jumpKey: building.aliases[0] + (room ? '-' + room : ''),
    buildingId: building.id,
    name: building.name,
    campus: building.campus,
    room,
    floor,
    titleText: room ? building.name + ' · ' + room + ' 教室' : building.name,
    floorText: floor ? floor + ' 楼' : '—',
    roomText: room || '未指定房间',
    entrance: building.entrance,
    stairs: building.stairs,
    walkMin: building.walkMin,
    bikeMin: building.bikeMin,
    advice: building.advice,
    note: building.note,
    // 室内导航支持情况：
    // hasIndoorMap —— 这栋楼有没有数字化平面图（与 Rust 后端数据同步）
    // roomNodeId   —— 房间在图中的节点 id，算路接口的输入
    hasIndoorMap: Boolean(graph),
    roomNodeId: roomNodeId
  }
}

module.exports = { resolvePlace, normalize, floorOf }
