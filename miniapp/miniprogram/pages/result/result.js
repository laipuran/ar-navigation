// 引入地点词典模块。注意 require 一定要写在最上面：
// const 声明的变量在声明之前是"不能用的"（暂时性死区），
// 所以别把 require 放到文件末尾去。
const { resolvePlace } = require('../../data/places.js')

Page({
  // data 里只放"页面要显示的东西"
  data: {
    raw: '',        // 用户原始输入，比如 "教四-302"
    result: null,   // 解析成功后的结构化数据
    notFound: false // 是否没查到（决定显示哪块内容）
  },

  // options 是上一页通过 url 传过来的参数，比如 ?place=%E6%95%99%E5%9B%9B-302
  onLoad(options) {
    // 传过来的中文是编码过的，必须 decodeURIComponent 解码还原
    const raw = decodeURIComponent(options.place || '')

    // 拿到原始输入后，交给词典模块去解析
    const result = resolvePlace(raw)

    this.setData({
      raw: raw,
      result: result.ok ? result : null,
      notFound: !result.ok
    })

    // 顺手把页面标题改成地点名，用户一眼知道自己在看什么
    if (result.ok) {
      wx.setNavigationBarTitle({ title: result.name })
    }
  },

  // 点击"开始导航" -> 三维地图页
  onStartNav() {
    const place = this.data.result
    if (!place) return
    wx.navigateTo({
      url: '/pages/map/map?place=' + encodeURIComponent(place.jumpKey)
    })
  },

  // 点击"AR 实景指引" -> AR 页
  onStartAR() {
    const place = this.data.result
    if (!place) return
    wx.navigateTo({
      url: '/pages/ar/ar?place=' + encodeURIComponent(place.jumpKey)
    })
  },

  // 返回上一页，让用户可以接着查别的
  onBack() {
    wx.navigateBack()
  }
})
