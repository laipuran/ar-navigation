// timetable.js —— 课表识别页
//
// 【流程】选图（首页做完了） -> 传到这一页 -> 识别 -> 显示课程列表 -> 点某节课跳到结果页
//
// 【状态机】status 只有两个值：
//   loading：正在识别，显示进度条
//   done：识别完成，显示列表
// 用"状态机"而不是一堆布尔变量（isLoading / isDone / hasError...）来管界面，
// 这样永远不会出现"两个状态同时为真"的诡异情况。

const { resolvePlace } = require('../../data/places.js')
const { recognize } = require('../../services/ocr.js')

Page({
  data: {
    imagePath: '',    // 用户选的课表照片（真显示，不是占位图）
    status: 'loading',
    progress: 0,      // 0~100
    courses: [],      // 解析后的课程列表
    knownCount: 0,    // 能解析出位置的门数
    unknownCount: 0,  // 词库未收录的门数
    isMock: true      // 是否走的是模拟识别
  },

  onLoad(options) {
    // 上一页通过 url 传来的图片临时路径
    const imagePath = decodeURIComponent(options.image || '')
    this.setData({ imagePath })
    this.start()
  },

  // 离开页面必须清定时器，否则进度条会在后台空转
  onUnload() {
    this.stopTimers()
  },

  stopTimers() {
    if (this.progressTimer) {
      clearInterval(this.progressTimer)
      this.progressTimer = null
    }
    if (this.doneTimer) {
      clearTimeout(this.doneTimer)
      this.doneTimer = null
    }
  },

  // ---------- 开始识别 ----------
  start() {
    this.setData({ status: 'loading', progress: 0, courses: [] })
    this.runProgress()

    // recognize 返回 Promise，用 .then 而不是 async/await
    // （不是不能写 async，只是 .then 对"还要兼容老基础库"的场景更稳）
    recognize(this.data.imagePath).then((res) => {
      const courses = []
      let known = 0
      let unknown = 0

      // 对识别出的每一个地点，再做一次"能不能解析"的检查
      // 这一步很关键：识别出来 ≠ 我们认识它，必须分开处理
      for (let i = 0; i < res.courses.length; i++) {
        const c = res.courses[i]
        const r = resolvePlace(c.place)
        if (r.ok) {
          known++
        } else {
          unknown++
        }
        courses.push({
          time: c.time,
          course: c.course,
          place: c.place,
          ok: r.ok,
          placeName: r.ok ? r.name : '词库未收录',
          jumpKey: r.ok ? r.jumpKey : ''
        })
      }

      this.stopTimers()
      this.setData({ progress: 100 })

      // 停一下再切状态，让用户看到 100%，不然进度条会"跳"到结束
      this.doneTimer = setTimeout(() => {
        this.setData({
          status: 'done',
          courses: courses,
          knownCount: known,
          unknownCount: unknown,
          isMock: res.engine === 'mock'
        })
      }, 450)
    }).catch(() => {
      // 真实接入后网络会失败，这里先占好位置
      this.stopTimers()
      this.setData({ status: 'done', courses: [], isMock: true })
      wx.showToast({ title: '识别失败，请重试', icon: 'none' })
    })
  },

  // 进度条动画：越接近 92% 走得越慢，模拟"正在处理"的感觉
  runProgress() {
    this.progressTimer = setInterval(() => {
      const current = this.data.progress
      if (current >= 92) return
      // 剩余距离的十分之一，至少加 1，保证一定会前进
      let next = current + Math.max(1, Math.ceil((92 - current) / 10))
      if (next > 92) next = 92
      this.setData({ progress: next })
    }, 130)
  },

  // 点某一节课
  onCourseTap(e) {
    const index = e.currentTarget.dataset.index
    const item = this.data.courses[index]

    if (!item.ok) {
      wx.showToast({ title: '「' + item.place + '」还没收录', icon: 'none' })
      return
    }

    wx.navigateTo({
      url: '/pages/result/result?place=' + encodeURIComponent(item.jumpKey)
    })
  },

  // 重拍 / 重新选图
  onRetake() {
    wx.chooseMedia({
      count: 1,
      mediaType: ['image'],
      sourceType: ['album', 'camera'],
      success: (res) => {
        const filePath = res.tempFiles[0].tempFilePath
        this.setData({ imagePath: filePath })
        this.start()
      }
    })
  }
})
