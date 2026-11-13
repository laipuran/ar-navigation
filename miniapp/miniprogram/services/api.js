// services/api.js —— 后端接口统一封装（BFF 客户端）
//
// 【为什么要有这一层】
// 页面不该直接拼 URL、处理 wx.request 的回调地狱。
// 这里把"调后端"封装成 Promise，页面只管 .then / .catch。
// 以后后端地址变了、或者换成了同学 Rust 后端的正式接口，只改这个文件。
//
// 【开发期怎么用】
// 1. 在电脑上启动后端：cd miniapp/server && node server.js（端口 8787）
// 2. 开发者工具：右上角「详情」->「本地设置」-> 勾选「不校验合法域名」
//    （因为 127.0.0.1 不是 https 合法域名，勾选后开发阶段可以随便调）
// 3. 真机预览时，把 BASE_URL 换成电脑的局域网 IP（手机和电脑必须同一 WiFi）
//    例如：http://192.168.1.5:8787
//    查 IP：CMD 里输入 ipconfig，找"无线局域网适配器"下的 IPv4 地址

const BASE_URL = 'http://127.0.0.1:8787'

// 总开关：true = 优先走后端，失败自动回退本地数据；false = 全部本地
// 答辩演示怕断网时可以改成 false，瞬间降级为纯本地版
const remoteEnabled = true

// 通用请求：把 wx.request 包装成 Promise
function request(method, path, data, timeoutMs) {
  return new Promise(function (resolve, reject) {
    wx.request({
      url: BASE_URL + path,
      method: method,
      data: data,
      timeout: timeoutMs || 8000,
      success: function (res) {
        if (res.statusCode >= 200 && res.statusCode < 300) {
          resolve(res.data)
        } else {
          // 后端的错误结构是 { error: { code, message } }，原样抛给调用方
          const err = (res.data && res.data.error) || {}
          reject({
            code: err.code || 'HTTP_' + res.statusCode,
            message: err.message || ('请求失败：' + res.statusCode)
          })
        }
      },
      fail: function (err) {
        reject({ code: 'NETWORK', message: '连不上后端（' + BASE_URL + '）：' + (err.errMsg || '') })
      }
    })
  })
}

// ---------- 各接口的语义化封装 ----------

// 黑话解析："教四-302" -> 结构化地点信息
function resolvePlaceRemote(name) {
  return request('GET', '/api/v1/places/resolve?name=' + encodeURIComponent(name))
}

// 楼内路线："教四-302" -> 门到房间的分步指引
function planRoute(name) {
  return request('GET', '/api/v1/routes/plan?name=' + encodeURIComponent(name))
}

// 课表识别（当前后端也是 mock，但链路是真的）
function recognizeTimetable() {
  return request('POST', '/api/v1/ocr/recognize', {}, 4000)
}

// AI 对话：{ message, conversation_id? } -> { conversation_id, reply }
function sendChat(payload) {
  // 对话可能要等大模型，给足 30 秒
  return request('POST', '/api/v1/chat/messages', payload, 30000)
}

module.exports = {
  resolvePlaceRemote,
  planRoute,
  recognizeTimetable,
  sendChat,
  remoteEnabled,
  BASE_URL
}
