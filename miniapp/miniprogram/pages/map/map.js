// map.js —— 三维地图页（校园总览 + 室内楼层示意）
//
// 【为什么不是真三维】
// 团队方案里原本想用 Cesium 做三维地图，塞进 web-view。
// 但个人主体的小程序【不支持 web-view 组件】，这条路走不通。
// 所以这里用两个原生能力拼出"导航感"：
//   1. map 组件 —— 校园级：楼栋位置 + 从校门到楼的路线
//   2. view 拼的楼层示意 —— 室内级：走廊、楼梯、目标房间
//
// 【为什么室内只能用示意图】
// 商业地图（高德/百度）没有室内数据，室内平面图必须自己测绘。
// 这个页面把"结构"搭好了，等 GIS 同学给了真实平面图，
// 把 floorplan 部分换成图片 + 坐标映射即可。

const { resolvePlace } = require('../../data/places.js')
const { findBuilding } = require('../../data/geo.js')

Page({
  data: {
    place: null,
    center: { lat: 30.5400, lng: 114.3580 },
    markers: [],
    polyline: [],
    hasGeo: false,
    gateName: '',
    roomsTop: [],
    roomsBottom: []
  },

  onLoad(options) {
    const raw = decodeURIComponent(options.place || '')
    const result = resolvePlace(raw)

    if (!result.ok) {
      wx.showToast({ title: '这个地点还没收录', icon: 'none' })
      return
    }

    this.setData({ place: result })
    wx.setNavigationBarTitle({ title: result.name })
    this.buildMap(result)
    this.buildFloor(result)
  },

  // ---------- 校园级：地图 + 路线 ----------
  buildMap(place) {
    const building = findBuilding(place.name)

    // 楼栋没有坐标就老实说，别硬画一个假位置
    if (!building) {
      this.setData({ hasGeo: false })
      return
    }

    const markers = [
      {
        id: 1,
        latitude: building.gate.lat,
        longitude: building.gate.lng,
        callout: {
          content: building.gate.name,
          color: '#5F5E5A',
          fontSize: 11,
          borderRadius: 6,
          padding: 6,
          display: 'ALWAYS'
        }
      },
      {
        id: 2,
        latitude: building.lat,
        longitude: building.lng,
        callout: {
          content: place.name,
          color: '#185FA5',
          fontSize: 12,
          borderRadius: 6,
          padding: 6,
          display: 'ALWAYS'
        }
      }
    ]

    // 从校门到楼的一条线，arrowLine 会在末端加箭头
    const polyline = [{
      points: [
        { latitude: building.gate.lat, longitude: building.gate.lng },
        { latitude: building.lat, longitude: building.lng }
      ],
      color: '#185FA5CC',
      width: 6,
      arrowLine: true
    }]

    this.setData({
      center: { lat: building.lat, lng: building.lng },
      markers: markers,
      polyline: polyline,
      hasGeo: true,
      gateName: building.gate.name
    })
  },

  // ---------- 室内级：楼层示意 ----------
  buildFloor(place) {
    const roomNo = parseInt(place.room, 10)

    // 没给房间号就不画楼层图
    if (!roomNo) {
      this.setData({ roomsTop: [], roomsBottom: [] })
      return
    }

    this.setData({
      roomsTop: [
        { no: String(roomNo - 1), target: false },
        { no: place.room, target: true },
        { no: String(roomNo + 1), target: false }
      ],
      roomsBottom: [
        { no: String(roomNo + 2), target: false },
        { no: String(roomNo + 3), target: false },
        { no: String(roomNo + 4), target: false }
      ]
    })
  },

  // 跳 AR 实景页
  onOpenAR() {
    const place = this.data.place
    if (!place) return
    wx.navigateTo({
      url: '/pages/ar/ar?place=' + encodeURIComponent(place.jumpKey)
    })
  }
})
