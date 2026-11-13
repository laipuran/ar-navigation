// lib/buildings.js —— 楼栋数据加载
//
// 数据来源：data/buildings.json（楼栋索引）+ data/*.json（单栋楼的图）
// 格式与仓库 schemas/navigation.schema.json 完全一致，
// 也就是说：这份楼栋数据未来可以直接被 Rust 后端的 navigation::loader 读取，
// 两端共用同一份数据，不会出现"小程序说 302 在 3 楼、后端说在 4 楼"的分裂。
//
// 为什么启动时一次性加载、不做每次读盘？
// 楼栋数据是低频变更的静态数据，进程内缓存即可；数据更新后重启服务生效。

const fs = require('fs')
const path = require('path')

const DATA_DIR = path.join(__dirname, '..', 'data')

// 楼栋索引：元数据（名称/别名/出入口建议）+ 图数据文件指针
const INDEX = JSON.parse(fs.readFileSync(path.join(DATA_DIR, 'buildings.json'), 'utf8'))

// 逐栋加载图数据；file 为 null 表示"暂无室内图，只有基础信息"
const GRAPHS = new Map()
for (const item of INDEX) {
  if (item.file) {
    const raw = JSON.parse(fs.readFileSync(path.join(DATA_DIR, item.file), 'utf8'))
    GRAPHS.set(item.id, raw)
  }
}

// 构建查询辅助结构：id -> 节点
const NODE_MAPS = new Map()
for (const [id, graph] of GRAPHS) {
  const map = new Map()
  for (const node of graph.nodes) map.set(node.id, node)
  NODE_MAPS.set(id, map)
}

// 取全部楼栋的基础信息（不含图，避免响应过大）
function listBuildings() {
  return INDEX.map(function (b) {
    return {
      id: b.id,
      name: b.name,
      campus: b.campus,
      aliases: b.aliases,
      mapped: Boolean(b.file)   // 是否已有室内导航图
    }
  })
}

// 按别名或 id 找楼栋索引项
function findBuilding(key) {
  const k = (key || '').trim()
  if (!k) return null
  for (const b of INDEX) {
    if (b.id === k || b.name === k) return b
    for (const alias of b.aliases) {
      if (k.indexOf(alias) > -1 || alias.indexOf(k) > -1) return b
    }
  }
  return null
}

function getGraph(buildingId) {
  return GRAPHS.get(buildingId) || null
}

function getNodeMap(buildingId) {
  return NODE_MAPS.get(buildingId) || null
}

module.exports = { listBuildings, findBuilding, getGraph, getNodeMap }
