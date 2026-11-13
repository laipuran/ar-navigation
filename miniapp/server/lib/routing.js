// lib/routing.js —— 楼内最短路径 + 确定性指引
//
// 算法：Dijkstra（边权 = distance_m，无向图）。
// 这与 Rust 后端 navigation::routing 的语义保持一致（都是最短行走距离），
// 但注意：后端的算路是进程内实现、没有 REST 接口（见 docs/specs/backend-internal.md），
// 所以小程序需要的"门到房间"路线在这里用同一份数据独立实现。
// 未来如果后端开放了算路 REST 接口，把本文件的 planRoute 换成 HTTP 调用即可，
// server.js 的路由和响应结构不用动。
//
// 指引生成规则（对应 schema 里 junction.neighbors 的约定）：
// neighbors 按"俯视顺时针"排列。已知来路和去路在列表中的下标差，
// 可以推断转向：顺时针下一位 = 右转，逆时针一位 = 左转，正对面 = 直行。

const { getGraph, getNodeMap } = require('./buildings')

// ---------- 图构建 ----------
function buildAdjacency(graph) {
  const adj = new Map()
  for (const node of graph.nodes) adj.set(node.id, [])
  for (const edge of graph.edges) {
    const [a, b] = edge.endpoints
    adj.get(a).push({ to: b, meters: edge.kind.distance_m, kind: edge.kind.type })
    adj.get(b).push({ to: a, meters: edge.kind.distance_m, kind: edge.kind.type })
  }
  return adj
}

// ---------- Dijkstra ----------
// 返回 { path: [nodeId...], meters: 总距离 }；不可达返回 null
function shortestPath(adj, from, to) {
  const dist = new Map()
  const prev = new Map()
  const visited = new Set()

  dist.set(from, 0)

  while (true) {
    // 取当前未访问、距离最小的节点（堆优化对几十个节点的图没必要，线性扫足够）
    let best = null
    let bestDist = Infinity
    for (const [id, d] of dist) {
      if (!visited.has(id) && d < bestDist) {
        best = id
        bestDist = d
      }
    }
    if (best === null) break
    if (best === to) break
    visited.add(best)

    for (const edge of adj.get(best) || []) {
      const nd = bestDist + edge.meters
      if (nd < (dist.get(edge.to) ?? Infinity)) {
        dist.set(edge.to, nd)
        prev.set(edge.to, best)
      }
    }
  }

  if (!dist.has(to)) return null

  // 回溯路径
  const path = []
  let cur = to
  while (cur !== undefined) {
    path.unshift(cur)
    cur = prev.get(cur)
    if (cur === undefined && path[0] !== from) break
  }
  return { path, meters: dist.get(to) }
}

// ---------- 转向推断 ----------
// junction.neighbors 俯视顺时针；来路不计入出口序号（schema 约定）
function turnAt(nodeMap, junction, incomingId, outgoingId) {
  const neighbors = junction.kind.neighbors
  const inIdx = neighbors.indexOf(incomingId)
  const outIdx = neighbors.indexOf(outgoingId)
  if (inIdx === -1 || outIdx === -1) return null
  const n = neighbors.length
  const diff = (outIdx - inIdx + n) % n
  if (diff === 0) return null
  if (diff === 1) return '右转'
  if (diff === n - 1) return '左转'
  if (n % 2 === 0 && diff === n / 2) return '直行'
  return '斜向通过'
}

// ---------- 指引生成 ----------
// 把节点序列翻译成一步一步的中文指引
function buildSteps(nodeMap, graph, path, targetName) {
  const steps = []
  let walk = null   // 累积中的走廊段 { meters, turn }

  function flushWalk() {
    if (walk && walk.meters > 0) {
      let text = '沿走廊走 ' + walk.meters + ' 米'
      if (walk.turn) text += '，然后' + walk.turn
      steps.push({ type: 'walk', text, meters: walk.meters, turn: walk.turn || null })
    }
    walk = null
  }

  for (let i = 0; i < path.length - 1; i++) {
    const aId = path[i]
    const bId = path[i + 1]
    const a = nodeMap.get(aId)
    const b = nodeMap.get(bId)

    // 找到 a-b 这条边
    const graph_edge = graph.edges.find(function (e) {
      const [x, y] = e.endpoints
      return (x === aId && y === bId) || (x === bId && y === aId)
    })

    if (graph_edge.kind.type === 'walk') {
      if (!walk) walk = { meters: 0, turn: null }
      walk.meters += graph_edge.kind.distance_m
    } else {
      // stair_flight：楼梯段
      flushWalk()
      const up = b.floor_id > a.floor_id
      steps.push({
        type: 'stairs',
        text: (up ? '上楼梯到 ' : '下楼梯到 ') + b.floor_id + ' 楼',
        meters: graph_edge.kind.distance_m,
        fromFloor: a.floor_id,
        toFloor: b.floor_id
      })
    }

    // 路径中途经过转角节点 -> 推断转向，附加到当前走廊段
    const nextId = path[i + 2]
    if (nextId && b.kind.type === 'junction') {
      const turn = turnAt(nodeMap, b, aId, nextId)
      if (turn) {
        if (walk) walk.turn = turn
        else steps.push({ type: 'turn', text: turn, meters: 0 })
      }
    }
  }
  flushWalk()

  steps.push({ type: 'arrive', text: '到达 ' + targetName, meters: 0 })
  return steps
}

// ---------- 对外入口 ----------
// place: places.resolvePlace 的成功结果（需 hasIndoorMap 且 roomNodeId 非空）
function planRoute(place) {
  const graph = getGraph(place.buildingId)
  const nodeMap = getNodeMap(place.buildingId)
  if (!graph || !nodeMap) {
    return { ok: false, code: 'ROUTE_NOT_MAPPED', message: '这栋楼还没有室内导航图，暂不能算路。' }
  }
  if (!place.roomNodeId) {
    return { ok: false, code: 'ROOM_NOT_IN_GRAPH', message: '图里没有找到 ' + place.room + ' 教室，请核对房间号。' }
  }

  const adj = buildAdjacency(graph)
  const target = nodeMap.get(place.roomNodeId)

  // 从每个门各算一次最短路，取最短的（对应"从哪个门进"）
  const gates = graph.nodes.filter(function (n) { return n.kind.type === 'gate' })
  let best = null
  for (const gate of gates) {
    const result = shortestPath(adj, gate.id, place.roomNodeId)
    if (result && (!best || result.meters < best.meters)) best = result
  }

  if (!best) {
    return { ok: false, code: 'ROUTE_UNREACHABLE', message: '门与教室之间不存在连通路径，楼内数据可能缺失。' }
  }

  const steps = buildSteps(nodeMap, graph, best.path, target.kind.name)

  return {
    ok: true,
    place: place,
    totalMeters: Math.round(best.meters * 10) / 10,
    entranceNodeId: best.path[0],
    steps
  }
}

module.exports = { planRoute, shortestPath, buildAdjacency }
