// ar.js —— AR 实景指引页
//
// 【方案说明：为什么不是 VKSession 真 3D 锚定】
// 微信的 wx.createVKSession 能做平面识别 + 3D 叠加，但：
//   · 只有部分机型支持，安卓碎片化严重
//   · 开发者工具无法模拟，只能真机
//   · 首次使用要下载识别模型，弱网会失败
// 高德、百度步行 AR 导航的实际做法是"相机画面 + 2D 箭头叠加"，
// 也就是这一页的做法：稳定、所有机型可用、效果够看。
//
// 【以后要升级成真 3D】
// 在 onReady 里创建 VKSession，把箭头换成 WebGL canvas 绘制，
// 用 deviceMotion 或 VKSession 的相机位姿做锚定。其余 UI 不变。
//
// 【原生组件层级】
// camera 是原生组件，普通 view 盖不上去，必须用 cover-view 做叠加层。
// 这是小程序里唯一能可靠覆盖 camera 的写法。

const { resolvePlace } = require('../../data/places.js')

Page({
  data: {
    place: null,
    hint: '',
    distance: 120,
    heading: 0,        // 箭头旋转角度
    cameraError: false,
    simRunning: true
  },

  onLoad(options) {
    const raw = decodeURIComponent(options.place || '')
    const result = resolvePlace(raw)

    if (!result.ok) {
      wx.showToast({ title: '这个地点还没收录', icon: 'none' })
      return
    }

    // 把"每一步该干什么"做成数组，演示时循环播放
    const steps = [
      { hint: '直行进入' + result.entrance, distance: 120 },
      { hint: '继续直走，找到楼梯口', distance: 60 },
      { hint: '上楼，前往 ' + result.floorText, distance: 25 },
      { hint: result.roomText + ' 就在走廊尽头', distance: 8 }
    ]

    this.steps = steps
    this.stepIndex = 0

    this.setData({
      place: result,
      hint: steps[0].hint,
      distance: steps[0].distance
    })
    wx.setNavigationBarTitle({ title: 'AR 实景指引' })

    this.startSim()
  },

  // 离开页面必须清掉所有定时器
  onUnload() {
    this.stopSim()
  },

  startSim() {
    this.stopSim()

    // 每 4 秒推进到下一步
    this.stepTimer = setInterval(() => {
      this.stepIndex = (this.stepIndex + 1) % this.steps.length
      const step = this.steps[this.stepIndex]
      this.setData({ hint: step.hint, distance: step.distance })
    }, 4000)

    // 每 200ms 微调箭头角度，模拟手持设备时的方向抖动
    // 用正弦波，看起来像呼吸一样自然
    this.swayTimer = setInterval(() => {
      const angle = Math.sin(Date.now() / 900) * 14
      this.setData({ heading: Math.round(angle) })
    }, 200)
  },

  stopSim() {
    if (this.stepTimer) {
      clearInterval(this.stepTimer)
      this.stepTimer = null
    }
    if (this.swayTimer) {
      clearInterval(this.swayTimer)
      this.swayTimer = null
    }
  },

  // 暂停 / 继续演示（答辩时想停下来讲解，用得上）
  onToggleSim() {
    if (this.data.simRunning) {
      this.stopSim()
      this.setData({ simRunning: false })
    } else {
      this.startSim()
      this.setData({ simRunning: true })
    }
  },

  // 相机启动失败（开发者工具里几乎必然触发）
  onCameraError() {
    this.setData({ cameraError: true })
  },

  onExit() {
    wx.navigateBack()
  }
})
