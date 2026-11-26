// 结果页：黑话解析结果的展示
//
// 【数据来源策略：远程优先、本地兜底】
// 页面先用本地词典（data/places.js）立刻渲染，保证"永远有东西看"；
// 同时如果后端（miniapp/server）在运行，就请求远程解析结果，
// 成功后用服务端数据覆盖（服务端数据与 Rust 后端同源，还会带楼内导航信息）。
// 这样：断网/后端没启动 -> 页面照常工作；后端在跑 -> 数据更准。
const { resolvePlace } = require('../../data/places.js')
const { resolvePlaceRemote, remoteEnabled } = require('../../services/api.js')

Page({
  // data 里只放"页面要显示的东西"
  data: {
    raw: '',        // 用户原始输入，比如 "教四-302"
    result: null,   // 解析成功后的结构化数据
    notFound: false, // 是否没查到（决定显示哪块内容）
    source: ''      // 数据来源标记：'local' | 'server'，仅用于调试
  },

  // options 是上一页通过 url 传过来的参数，比如 ?place=%E6%95%99%E5%9B%9B-302
  onLoad(options) {
    // 传过来的中文是编码过的，必须 decodeURIComponent 解码还原
    const raw = decodeURIComponent(options.place || '')

    // 第 1 步：本地解析，立刻渲染（同步，永不失败）
    this.applyResult(resolvePlace(raw), 'local')

    // 第 2 步：远程解析（异步），成功则覆盖
    // 注意箭头函数保留了外层的 this，如果换成 function(){}，this 就不是页面实例了
    if (remoteEnabled) {
      resolvePlaceRemote(raw)
        .then((remote) => {
          if (remote.ok) this.applyResult(remote, 'server')
        })
        .catch(() => {
          // 后端没启动 / 断网：静默保留本地结果，不打扰用户
          console.log('[result] 远程解析不可用，已使用本地数据')
        })
    }
  },

  // 统一处理"往页面里放结果"：本地和远程返回的字段结构是一致的
  applyResult(result, source) {
    const ok = result && result.ok
    this.setData({
      raw: this.data.raw || result.raw,
      result: ok ? result : null,
      notFound: !ok,
      source: ok ? source : ''
    })
    // 顺手把页面标题改成地点名，用户一眼知道自己在看什么
    if (ok) {
      wx.setNavigationBarTitle({ title: result.name })
    }
  },

  // 点击"开始导航" -> 地图页
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
