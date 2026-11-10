// data/geo.js —— 校园地理坐标表
//
// 【重要】下面的经纬度是"近似示例值"，用于让地图页能跑起来。
// 真实坐标要等校园 GIS 数据，或者你在高德/腾讯地图上逐个点选后替换。
// 替换时只改这个文件，页面代码不用动。
//
// gate 字段 = 离这栋楼最近的校门。地图上会从校门画一条线到楼，
// 这就是"校园级导航"那一段；进楼之后的"室内导航"由楼层示意图表达。

const BUILDINGS = [
  {
    name: '第四教学楼',
    lat: 30.5398,
    lng: 114.3596,
    gate: { lat: 30.5386, lng: 114.3610, name: '珞珈门' }
  },
  {
    name: '第五教学楼',
    lat: 30.5410,
    lng: 114.3610,
    gate: { lat: 30.5398, lng: 114.3622, name: '樱花大道门' }
  },
  {
    name: '工学部主教学楼',
    lat: 30.5478,
    lng: 114.3520,
    gate: { lat: 30.5462, lng: 114.3538, name: '工学部东门' }
  },
  {
    name: '信息学部第一教学楼',
    lat: 30.5320,
    lng: 114.3480,
    gate: { lat: 30.5310, lng: 114.3495, name: '信息学部北门' }
  }
]

// 按楼名查坐标，查不到返回 null
function findBuilding(name) {
  for (let i = 0; i < BUILDINGS.length; i++) {
    if (BUILDINGS[i].name === name) {
      return BUILDINGS[i]
    }
  }
  return null
}

module.exports = {
  findBuilding: findBuilding,
  BUILDINGS: BUILDINGS
}
