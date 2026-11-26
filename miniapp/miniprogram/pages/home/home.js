// home.js：首页的逻辑。记住核心循环：改 data -> setData -> 界面自动更新

// 历史记录存在本地的"钥匙"，读写要用同一个 key
const STORAGE_KEY = 'nav_history'
// 最多保留几条历史
const MAX_HISTORY = 20

Page({
  // data：页面的"状态"。wxml 里 {{ }} 引用的变量都来自这里
  data: {
    inputValue: '',   // 输入框当前内容
    history: []       // 历史记录数组，每项 { id, name, timeText }
  },

  // 页面加载时触发，只执行一次，适合读取本地存储
  onLoad() {
    this.loadHistory()
  },

  // ---------- 工具方法：读/写本地存储 ----------
  // wx.getStorageSync / wx.setStorageSync 是同步的本地存储 API，类似 localStorage
  loadHistory() {
    const history = wx.getStorageSync(STORAGE_KEY) || []
    this.setData({ history })
  },

  saveHistory(history) {
    // 存储只接受简单数据，超长的话截断
    wx.setStorageSync(STORAGE_KEY, history.slice(0, MAX_HISTORY))
  },

  // ---------- 事件处理：wxml 里 bindtap="xxx" 对应这里的 xxx 方法 ----------

  // 输入框每敲一个字触发一次，e.detail.value 是最新内容
  onInput(e) {
    this.setData({ inputValue: e.detail.value })
  },

  // 点击"拍照识课表"
  onChooseTimetable() {
    wx.chooseMedia({
      count: 1,                      // 一次选一张
      mediaType: ['image'],          // 只要图片
      sourceType: ['album', 'camera'], // 相册和拍照都允许
      success: (res) => {
        // res.tempFiles[0].tempFilePath 是选中图片的临时路径
        const filePath = res.tempFiles[0].tempFilePath

        // 把图片路径传给我下一页去识别
        // 路径里可能有特殊字符，所以同样要编码
        wx.navigateTo({
          url: '/pages/timetable/timetable?image=' + encodeURIComponent(filePath)
        })
      }
    })
  },

  // 点击"问问 AI 助手" -> 进入对话页
  onOpenChat() {
    wx.navigateTo({ url: '/pages/chat/chat' })
  },

  // 点击"查询"或键盘上的搜索键
  onSearch() {
    // js 里用 trim() 去掉首尾空格，避免存进一堆空格
    const name = this.data.inputValue.trim()
    if (!name) {
      // wx.showToast：轻提示，icon: 'none' 表示纯文字
      wx.showToast({ title: '先输入一个地点哦', icon: 'none' })
      return
    }

    const now = new Date()
    // 手动拼时间字符串：wxml 里不能调用函数，所以格式化要在这里做完
    const timeText = `${now.getMonth() + 1}月${now.getDate()}日 ${String(now.getHours()).padStart(2, '0')}:${String(now.getMinutes()).padStart(2, '0')}`

    // 新记录插到最前面（最新的排在上面）
    const history = [{ id: Date.now(), name, timeText }, ...this.data.history]

    this.setData({
      history,
      inputValue: '' // 查完自动清空输入框，体验更好
    })
    this.saveHistory(history)

    // 跳转到结果页，把地名带过去
    // encodeURIComponent 必须加：url 里不能直接放中文和特殊符号，要先编码
    // 结果页再用 decodeURIComponent 还原
    wx.navigateTo({
      url: '/pages/result/result?place=' + encodeURIComponent(name)
    })
  },

  // 点击某条历史。data-index 在 wxml 里标了序号，这里取出来
  onItemTap(e) {
    const index = e.currentTarget.dataset.index
    const item = this.data.history[index]
    // 点历史记录 = 重新查一次，同样跳结果页
    wx.navigateTo({
      url: '/pages/result/result?place=' + encodeURIComponent(item.name)
    })
  },

  // 长按某条历史 -> 询问是否删除
  onItemLongPress(e) {
    const index = e.currentTarget.dataset.index
    const item = this.data.history[index]
    wx.showModal({
      title: '删除记录',
      content: `确定删除「${item.name}」吗？`,
      confirmColor: '#a32d2d',
      success: (res) => {
        if (res.confirm) {
          // filter：把下标不是 index 的都留下，实现"删除第 index 项"
          const history = this.data.history.filter((_, i) => i !== index)
          this.setData({ history })
          this.saveHistory(history)
        }
      }
    })
  },

  // 清空全部历史
  onClearHistory() {
    wx.showModal({
      title: '清空记录',
      content: '确定清空全部历史吗？',
      confirmColor: '#a32d2d',
      success: (res) => {
        if (res.confirm) {
          this.setData({ history: [] })
          this.saveHistory([])
        }
      }
    })
  }
})
