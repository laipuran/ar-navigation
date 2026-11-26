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
    cameraErrMsg: '',  // 相机失败的具体原因（排查用）
    camAuth: 'unknown', // scope.camera 授权状态：granted / denied / prompt / unknown
    camDebug: '',       // 排查用：基础库版本 + 完整授权表
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

    this.checkCameraAuth()
    this.startSim()
  },

  // 主动查询相机授权状态，分三种情况处理：
  //   granted —— 已授权，等 camera 组件自然启动即可
  //   prompt  —— 从未问过，主动调 wx.authorize 触发系统弹窗
  //   denied  —— 曾经被拒，弹窗不会再出现，只能引导用户去设置页手动开
  checkCameraAuth() {
    wx.getSetting({
      success: (res) => {
        const auth = res.authSetting['scope.camera']
        // 把基础库版本和完整授权表打到界面上，便于真机一眼定位问题
        let sdk = '未知'
        try { sdk = wx.getSystemInfoSync().SDKVersion } catch (err) {}
        this.setData({
          camDebug: '基础库 ' + sdk + '｜授权表 ' + JSON.stringify(res.authSetting)
        })
        if (auth === true) {
          this.setData({ camAuth: 'granted' })
        } else if (auth === false) {
          // 曾经点过"拒绝"：记录状态，wxml 里会显示"去授权"按钮
          this.setData({ camAuth: 'denied' })
          console.warn('相机权限曾被拒绝，需引导用户到设置页开启')
        } else {
          // undefined = 从未询问过，主动发起授权请求（会弹窗）
          this.setData({ camAuth: 'prompt' })
          wx.authorize({
            scope: 'scope.camera',
            success: () => this.setData({ camAuth: 'granted' }),
            fail: (e) => {
              // 用户在弹窗里点了"拒绝"也会走到这里
              this.setData({ camAuth: 'denied' })
              console.warn('wx.authorize 相机授权被拒：', e.errMsg)
            }
          })
        }
      },
      fail: (e) => console.error('getSetting 失败：', e.errMsg)
    })
  },

  // 跳到小程序的授权设置页（用户手动打开相机开关后返回）
  onOpenAuthSetting() {
    wx.openSetting({
      success: (res) => {
        if (res.authSetting['scope.camera'] === true) {
          // 用户开好了：复位降级界面，重新挂载 camera 组件
          this.setData({ camAuth: 'granted', cameraError: false, cameraErrMsg: '' })
          wx.showToast({ title: '相机权限已开启', icon: 'success' })
        } else {
          wx.showToast({ title: '相机权限仍未开启', icon: 'none' })
        }
      }
    })
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
  // e.detail.errMsg 会告诉我们败在哪一层：系统权限 / 微信授权 / 隐私协议 / 组件错误
  onCameraError(e) {
    const detail = (e && e.detail) || {}
    const msg = detail.errMsg || JSON.stringify(detail)
    console.error('相机启动失败：', msg)
    this.setData({ cameraError: true, cameraErrMsg: msg })
  },

  onExit() {
    wx.navigateBack()
  }
})
