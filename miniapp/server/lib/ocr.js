// lib/ocr.js —— 课表识别（当前为模拟实现）
//
// 与小程序端 services/ocr.js 的 mock 数据完全一致，
// 这样前端无论调本地 mock 还是调本接口，看到的课表都相同。
//
// 【真实接入路径】（自建后端代理，推荐）
//   1. 在腾讯云控制台开通「通用印刷体识别」，拿到 SecretId / SecretKey
//   2. 小程序用 wx.uploadFile 把图片 POST 到 /api/v1/ocr/recognize（multipart）
//   3. 这里用 tencentcloud-sdk-nodejs 调 OCR 拿到文本
//   4. 用正则把文本切成 [{ time, course, place }]（见下方 parseTimetableText 的占位）
//   5. 密钥只存在服务端环境变量里，绝不放进小程序代码
//
// 没有配置密钥（TENCENT_OCR_SECRET_ID 缺失）时，本接口返回 mock 数据，
// 字段结构不变，前端可以无缝切换。

const SAMPLE_COURSES = [
  { time: '周一 08:00 - 09:35', course: '高等数学', place: '教四-302' },
  { time: '周一 10:05 - 11:40', course: '大学物理', place: '教五-101' },
  { time: '周二 14:05 - 15:40', course: '程序设计基础', place: '信部一教1201' },
  { time: '周三 08:00 - 09:35', course: '线性代数', place: '教四-401' },
  { time: '周四 10:05 - 11:40', course: '中国近现代史纲要', place: '明工教305' },
  { time: '周五 14:05 - 15:40', course: '体育（羽毛球）', place: '工主教405' }
]

// TODO: 接入真 OCR 后，在这里实现"识别文本 -> 课程列表"的解析。
// 思路：按行切分，每行用正则抓 "周X 第N-M节 / 时间段" + 课程名 + 地点。
function parseTimetableText(text) {
  void text
  return []
}

function recognize() {
  const hasKeys = Boolean(process.env.TENCENT_OCR_SECRET_ID && process.env.TENCENT_OCR_SECRET_KEY)
  if (!hasKeys) {
    return { ok: true, engine: 'mock', courses: SAMPLE_COURSES }
  }
  // 真实识别逻辑待接入；接入前即便有密钥也返回 mock，避免半成品上线
  return { ok: true, engine: 'mock', courses: SAMPLE_COURSES }
}

module.exports = { recognize, parseTimetableText }
