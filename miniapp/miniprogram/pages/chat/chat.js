// chat.js —— AI 对话页
//
// 【这一页的技术看点】
// 1. 打字机效果：用 setInterval 逐字更新一条消息的 content
// 2. 局部 setData：只改 messages[3].content，不重传整个数组（小程序特有语法）
// 3. 上下文：记住"上一句聊的是哪个地点"，才能听懂"走路要多久"这种追问
//
// 【关于"AI"】
// 现在没有后端，回复是靠本地规则生成的（调用 data/places.js 的解析器）。
// 以后接入真大模型时，只需要把 buildReply 换成 wx.request 请求后端，
// 打字机、滚动、上下文这些 UI 部分一行都不用改。

const { resolvePlace } = require('../../data/places.js')

// 开场白
const WELCOME = '你好，我是珞珈导航助手。\n把课表上的地点直接发给我，我帮你翻译成"人话"。\n比如：教四-302'

// 快捷问题按钮
const QUICK_LIST = ['教四-302 在哪', '走路要多久', '怎么走', '工主教 405']

// ---------- 从一句话里"挖出"地点 ----------
// 用户不会规规矩矩只输入"教四-302"，可能说"我想去教四302上课"，也可能带空格
// 正则拆解：
//   ([\u4e00-\u9fa5A-Za-z]+)  一段中文或字母 —— 楼宇名（贪婪匹配，会尽量多吃）
//   [\s\-－—_]*              中间允许有 0 到多个空格或各种横线
//   (\d{3,4})                3~4 位数字 —— 房间号
// 说明：resolvePlace 内部用的是"包含"匹配，所以"我想去教四"也能命中"教四"
function extractPlace(text) {
  const matched = text.match(/([\u4e00-\u9fa5A-Za-z]+)[\s\-－—_]*(\d{3,4})/)
  if (!matched) return null
  const result = resolvePlace(matched[1] + '-' + matched[2])
  return result.ok ? result : null
}

// ---------- 判断用户想知道什么 ----------
// 三个正则表达式就是三个"意图"
function wantTime(text) {
  return /多久|几分钟|时间|走路|步行|骑车|骑过去|远不远|远吗/.test(text)
}
function wantRoute(text) {
  return /怎么走|哪个门|楼梯|路线|怎么去|入口|进楼|进去/.test(text)
}
function wantFloor(text) {
  return /几楼|楼层|哪层|哪一层/.test(text)
}

// ---------- 根据意图生成回复 ----------
// 返回 { text: 回复内容, place: 本轮涉及的地点（用于记住上下文） }
function replyForPlace(place, text) {
  // 位置描述：有房间号才说楼层，否则只说校区
  const where = place.floor
    ? place.campus + ' · ' + place.floorText
    : place.campus

  if (wantTime(text) && !wantRoute(text)) {
    return {
      place: place,
      text: place.name + '：步行约 ' + place.walkMin + ' 分钟，骑行约 ' + place.bikeMin + ' 分钟。\n'
        + place.advice + '。'
    }
  }

  if (wantRoute(text)) {
    return {
      place: place,
      text: '去「' + place.titleText + '」这么走：\n'
        + '1. 从' + place.entrance + '进楼\n'
        + '2. ' + place.stairs + '\n'
        + '3. 上到 ' + (place.floorText === '—' ? '对应楼层' : place.floorText)
        + '，找到 ' + place.roomText
    }
  }

  if (wantFloor(text)) {
    return {
      place: place,
      text: place.floor
        ? place.titleText + ' 在 ' + place.floorText + '（房间号第一位就是楼层）。'
        : place.name + ' 在 ' + place.campus + '，你没说具体房间号，我不好判断楼层。'
    }
  }

  // 默认：给完整的"翻译卡"
  return {
    place: place,
    text: '找到啦：' + place.titleText + '\n'
      + '位置：' + where + '\n'
      + '进门：' + place.entrance + '\n'
      + '楼梯：' + place.stairs + '\n'
      + '耗时：步行 ' + place.walkMin + ' 分钟 / 骑行 ' + place.bikeMin + ' 分钟\n'
      + '建议：' + place.advice + '\n'
      + place.note
  }
}

// ---------- 主入口：一句话 -> 一段回复 ----------
// lastPlace 是"上一轮聊过的地点"，用来听懂追问
function buildReply(text, lastPlace) {
  const question = (text || '').trim()
  if (!question) {
    return { text: '你想问哪个地点？', place: lastPlace }
  }

  // 第 1 步：这句话里有没有地点
  const place = extractPlace(question)

  if (place) {
    // 有新地点：按意图回答，并把它记成当前上下文
    return replyForPlace(place, question)
  }

  // 第 2 步：没有新地点，但可能在追问上一轮的地点
  if (lastPlace && (wantTime(question) || wantRoute(question) || wantFloor(question))) {
    return replyForPlace(lastPlace, question)
  }

  // 第 3 步：在问时间/路线，但还没聊过任何地点 —— 得先要个地点才能答
  if ((wantTime(question) || wantRoute(question) || wantFloor(question)) && !lastPlace) {
    return {
      text: '你还没告诉我是哪个地点哦。\n先说一个教室，比如「教四-302」，我再帮你算时间和路线。',
      place: lastPlace
    }
  }

  // 第 4 步：纯闲聊 / 打招呼
  if (/你好|hi|hello|在吗|嗨/i.test(question)) {
    return { text: WELCOME, place: lastPlace }
  }

  // 第 4 步：兜底。这里明确承认"没收录"，比胡编一个答案好得多
  return {
    place: lastPlace,
    text: '这句我没太听懂，或者这个词库里还没有这个地点。\n'
      + '试试带上房间号，比如「教四-302」。\n'
      + '目前收录的有：教四、教五、工主教、信部一教。'
  }
}

Page({
  data: {
    messages: [],     // 消息列表，每项 { id, role, content, typing }
    inputValue: '',   // 输入框内容
    sending: false,   // 是否正在"打字"，防止连点
    quickList: QUICK_LIST,
    toView: ''        // scroll-view 要滚动到的元素 id
  },

  onLoad() {
    // 自增 id，保证每条消息的 id 唯一（wx:key 需要）
    this.msgId = 0
    // 当前上下文地点。放在 this 上而不是 data 里：
    // 因为它不需要显示，放 data 只是白白多一次渲染
    this.lastPlace = null
    // 打字定时器的句柄，页面卸载时要清掉
    this.timer = null

    // 推入开场白
    const messages = [{
      id: ++this.msgId,
      role: 'ai',
      content: WELCOME,
      typing: false,
      jumpKey: ''
    }]
    this.setData({ messages })
  },

  // 页面卸载时一定要清定时器，否则离开页面后它还在跑，造成内存泄漏
  onUnload() {
    if (this.timer) {
      clearInterval(this.timer)
      this.timer = null
    }
  },

  onInput(e) {
    this.setData({ inputValue: e.detail.value })
  },

  // 点快捷问题：先把文字填进输入框，再走同一套发送逻辑
  onQuickTap(e) {
    const question = e.currentTarget.dataset.q
    // setData 是异步的，必须写在回调里，否则 onSend 读到的还是旧值
    this.setData({ inputValue: question }, () => {
      this.onSend()
    })
  },

  // 点击气泡里的"查看完整信息" -> 跳结果页
  // 用 navigateTo 而不是 redirectTo：页面栈变成 home -> chat -> result，
  // 返回时能一层层退回来，聊天记录也还在
  onOpenResult(e) {
    const key = e.currentTarget.dataset.key
    if (!key) return
    wx.navigateTo({
      url: '/pages/result/result?place=' + encodeURIComponent(key)
    })
  },

  onSend() {
    const text = this.data.inputValue.trim()

    if (!text) {
      wx.showToast({ title: '说点什么吧', icon: 'none' })
      return
    }
    if (this.data.sending) {
      wx.showToast({ title: '等我说完这句', icon: 'none' })
      return
    }

    // 1. 先把"用户消息"和一条"空的 AI 消息"一起塞进列表
    //    AI 消息的 content 是空的，等着被逐字填满 —— 这就实现了"正在输入"
    const userMsg = { id: ++this.msgId, role: 'user', content: text, typing: false, jumpKey: '' }
    const aiMsg = { id: ++this.msgId, role: 'ai', content: '', typing: true, jumpKey: '' }

    // concat 生成新数组，而不是 push 改原数组
    // （改原数组 + setData 同一个数组，小程序有概率检测不到变化）
    const messages = this.data.messages.concat([userMsg, aiMsg])

    this.setData({ messages, inputValue: '', sending: true }, () => {
      this.scrollToBottom()
    })

    // 2. 算出回复内容（"AI 思考"的时间就用 setTimeout 假装一下）
    const reply = buildReply(text, this.lastPlace)
    this.lastPlace = reply.place   // 更新上下文

    // 3. 只有"回答了一个具体地点"时，才给跳转入口
    const jumpKey = reply.place && reply.place.jumpKey ? reply.place.jumpKey : ''

    // 延迟 400ms 再开始打字，比"秒回"更像真人
    setTimeout(() => {
      // 注意下标：新消息追加在末尾，所以是 messages.length - 1
      this.typeOut(messages.length - 1, reply.text, jumpKey)
    }, 400)
  },

  // ---------- 打字机 ----------
  // jumpKey：打字结束后要挂到消息上的跳转参数（空字符串表示不显示按钮）
  typeOut(index, fullText, jumpKey) {
    // Array.from 按"字符"拆，而不是 split('') 按 UTF-16 码元拆
    // 中文场景差别不大，但遇到 emoji 时 split('') 会把一个字拆成乱码
    const chars = Array.from(fullText)
    let cursor = 0

    this.timer = setInterval(() => {
      cursor += 1

      // 关键技巧：用 "messages[下标].字段" 这种路径只更新一条消息的某个字段
      // 如果每次都 this.setData({ messages: 整个数组 })，几十条消息会明显卡顿
      const patch = {}
      patch['messages[' + index + '].content'] = chars.slice(0, cursor).join('')
      patch['messages[' + index + '].typing'] = cursor < chars.length
      this.setData(patch)

      // 每打 8 个字滚动一次，不必每帧都滚（滚动本身也要重新渲染）
      if (cursor % 8 === 0) {
        this.scrollToBottom()
      }

      // 打完了
      if (cursor >= chars.length) {
        clearInterval(this.timer)
        this.timer = null

        const patch = { sending: false }
        // 打字结束才挂上跳转入口，避免气泡还没写完就冒出按钮
        if (jumpKey) {
          patch['messages[' + index + '].jumpKey'] = jumpKey
        }
        this.setData(patch)
        this.scrollToBottom()
      }
    }, 26)   // 26ms 一个字，一段 100 字约 2.6 秒
  },

  // ---------- 滚到底部 ----------
  // scroll-into-view 有一个坑：连续设成同一个值不会重复滚动。
  // 所以先清空、再用回调设成目标值，强制触发一次。
  scrollToBottom() {
    this.setData({ toView: '' }, () => {
      this.setData({ toView: 'anchor-bottom' })
    })
  }
})

// 【仅供本地测试】把回复函数导出，方便脱离小程序环境跑测试。
// 小程序运行时没有人 require 这个文件，所以这行不影响页面。
module.exports = { buildReply: buildReply }
