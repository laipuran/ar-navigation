// data/places.js —— 校园地点"黑话词典"（本地示例数据）
//
// 【重要说明】
// 下面这些地点的楼层、入口、楼梯位置都是"示例数据"，目的是让你先把
// "输入 -> 解析 -> 展示"这条链路跑通。真实信息需要等校园 GIS 数据 / 实地勘测
// 来校正（这也是赛题里"发榜单位提供数据支持"的部分）。
//
// 为什么单独放一个文件？
// 因为"数据"和"页面"要分开。以后接后端时，只要把这里换成
// wx.request 拿到的接口数据，result 页面的代码一行都不用改。
// 这个习惯叫"数据与视图分离"，是工程化的第一步。

// 词典本体：一个数组，每项是一个地点
// alias（别名）是关键词——用户可能输入"教四""四教""第四教学楼"，
// 只要命中任意一个别名，就能认出是这个地点
const PLACES = [
  {
    alias: ['教四', '四教', '教4', '第四教学楼'],
    name: '第四教学楼',
    campus: '文理学部',
    entrance: '教四南门（面向珞珈山路）',
    stairs: '进门左手边的东侧楼梯',
    walkMin: 15,
    bikeMin: 6,
    advice: '步行约 15 分钟，建议骑车',
    note: '上课前 10 分钟主干道人流密集，骑车请走外侧车道'
  },
  {
    alias: ['教五', '五教', '教5', '第五教学楼'],
    name: '第五教学楼',
    campus: '文理学部',
    entrance: '教五正门（樱花大道一侧）',
    stairs: '进门直走到底的中间楼梯',
    walkMin: 18,
    bikeMin: 7,
    advice: '步行约 18 分钟，强烈建议骑车',
    note: '樱花季正门区域限行，需从侧门绕行'
  },
  {
    alias: ['工主教', '工部主教', '工学部主教', '主教'],
    name: '工学部主教学楼',
    campus: '工学部',
    entrance: '工学部主教东门（靠近工学部一食堂）',
    stairs: '进门右转的北侧楼梯',
    walkMin: 22,
    bikeMin: 9,
    advice: '跨学部，步行 22 分钟太久，建议骑车或坐校车',
    note: '从文理学部到工学部要过东湖边的坡道，雨天路滑注意'
  },
  {
    alias: ['信息学部一教', '信部一教', '信一教', '国软'],
    name: '信息学部第一教学楼',
    campus: '信息学部',
    entrance: '信息学部一教北门',
    stairs: '进门左手边楼梯',
    walkMin: 25,
    bikeMin: 10,
    advice: '跨学部距离最远，建议骑车或乘校车',
    note: '校车 1 号线可直达，比骑车省力'
  }
]

// ---------- 工具函数 ----------

// 归一化：去掉所有空格（包括全角空格），把各种破折号统一成半角 "-"
// 用户输入的 "教四 - 302"、"教四－302"、"教四—302" 都能被正确识别
function normalize(text) {
  return (text || '')
    .replace(/[\s\u3000]/g, '')   // 半角空格 + 全角空格
    .replace(/[－—–_]/g, '-')     // 各种横线统一成 -
}

// 从房间号推算楼层：302 -> 3 楼，1201 -> 12 楼，102 -> 1 楼
// 规则：去掉最后两位（房间序号），剩下的就是楼层
function floorOf(room) {
  if (!room || room.length < 3) return null
  const num = parseInt(room.slice(0, room.length - 2), 10)
  if (isNaN(num)) return null
  return num === 0 ? 1 : num   // 002 这种，归到 1 楼
}

// ---------- 核心：把一个"黑话"解析成结构化信息 ----------
// 输入："教四-302"
// 输出：{ ok: true, name: '第四教学楼', campus: '文理学部', room: '302', floor: 3, ... }
function resolvePlace(input) {
  const raw = (input || '').trim()
  const key = normalize(raw)

  // 空输入直接失败
  if (!key) {
    return { ok: false, raw, reason: 'empty' }
  }

  // 第 1 步：把"楼宇"和"房间号"拆开
  // 正则解释：^(.+?)-?(\d{3,4})$
  //   (.+?)     最少一个字符，非贪婪——楼宇部分
  //   -?        中间可能有一个横线，也可能没有
  //   (\d{3,4}) 结尾的 3 到 4 位数字——房间号
  //   $         必须到结尾
  // 例："教四-302" -> 楼宇"教四"、房间"302"；"教四302" 同样能拆
  const matched = key.match(/^(.+?)-?(\d{3,4})$/)
  let buildingPart = key
  let room = ''
  if (matched) {
    buildingPart = matched[1]
    room = matched[2]
  }

  // 第 2 步：在词典里找楼宇
  // 命中规则：输入的楼宇部分"包含"某个别名，或某个别名"包含"输入
  // 这样 "教四附楼" 也能命中 "教四"
  let place = null
  for (let i = 0; i < PLACES.length; i++) {
    const item = PLACES[i]
    const hit = item.alias.some(function (a) {
      return buildingPart.indexOf(a) > -1 || a.indexOf(buildingPart) > -1
    })
    if (hit) {
      place = item
      break
    }
  }

  // 没找到 -> 返回失败，页面会显示"未收录"的兜底提示
  if (!place) {
    return { ok: false, raw, reason: 'not_found', buildingPart, room }
  }

  // 第 3 步：组装成页面能直接用的结构
  // 注意：wxml 里不能调用函数，所以所有"算出来的文字"都要在这里准备好
  const floor = floorOf(room)
  return {
    ok: true,
    raw,
    // jumpKey：一个"能再次被解析"的规范写法，比如 "教四-302"
    // 用途：别的页面要跳结果页时，直接用它拼 url，保证一定能解析成功
    jumpKey: place.alias[0] + (room ? '-' + room : ''),
    name: place.name,
    campus: place.campus,
    room,
    floor,
    // 展示用的文字，拼好给 wxml 直接渲染
    titleText: room ? place.name + ' · ' + room + ' 教室' : place.name,
    floorText: floor ? floor + ' 楼' : '—',
    roomText: room || '未指定房间',
    entrance: place.entrance,
    stairs: place.stairs,
    walkMin: place.walkMin,
    bikeMin: place.bikeMin,
    advice: place.advice,
    note: place.note
  }
}

// 小程序用 CommonJS 导出，别的文件 require 之后才能用
module.exports = {
  resolvePlace: resolvePlace,
  PLACES: PLACES
}
