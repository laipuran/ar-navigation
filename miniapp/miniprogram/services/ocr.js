// services/ocr.js —— 课表识别服务（可插拔）
//
// 【为什么单独抽一层"服务"】
// 因为"调接口"这件事，页面不该关心它怎么实现的。
// 页面只认一句话：recognize(图片路径) 返回一个 Promise，里面有 courses 数组。
// 现在这个 Promise 是本地 1.6 秒后返回假数据；
// 以后换成真的网络请求，页面代码一个字都不用改。
//
// ---------------------------------------------------------------
// 【怎么升级成真 OCR】—— 三条路，按可行性排序
// ---------------------------------------------------------------
// 路线 A（最终交付推荐）：自建后端代理
//   1. 后端（Node/Python）存 SecretId/SecretKey，调腾讯云 OCR 的"通用印刷体识别"
//   2. 小程序 wx.uploadFile 把图片传到后端，后端返回识别文本
//   3. 后端把识别出的文本用正则切分成 [{time, course, place}] 再返回
//   示例：
//     wx.uploadFile({
//       url: 'https://你的域名/api/ocr',
//       filePath: filePath,
//       name: 'file',
//       success: (res) => resolve(JSON.parse(res.data))
//     })
//
// 路线 B：微信云开发（需要正式 AppID，测试号不行）
//   1. 开发者工具左侧点「云开发」开通环境
//   2. 新建云函数 ocr，里面用 tencentcloud-sdk-nodejs 调 OCR
//   3. 前端 wx.cloud.callFunction({ name: 'ocr', data: { fileID } })
//
// 路线 C：纯前端方案（不推荐，准确率低）
//   小程序没有内置 OCR，浏览器端的 tesseract.js 又跑不进小程序沙箱，
//   只能做"图片裁剪 + 人工确认"，达不到演示效果。
// ---------------------------------------------------------------

// 模拟的课表识别结果
// 刻意混了两个"未收录"的地点，用来验证降级提示（真实场景一定会遇到）
const SAMPLE_COURSES = [
  { time: '周一 08:00 - 09:35', course: '高等数学', place: '教四-302' },
  { time: '周一 10:05 - 11:40', course: '大学物理', place: '教五-101' },
  { time: '周二 14:05 - 15:40', course: '程序设计基础', place: '信部一教1201' },
  { time: '周三 08:00 - 09:35', course: '线性代数', place: '教四-401' },
  { time: '周四 10:05 - 11:40', course: '中国近现代史纲要', place: '明工教305' },
  { time: '周五 14:05 - 15:40', course: '体育（羽毛球）', place: '工主教405' }
]

// 对外唯一接口
// 返回 Promise，resolve 出来的结构固定为 { ok, courses, engine }
// engine 字段用来标记"这次是模拟还是真识别"，方便页面显示提示
function recognize(filePath) {
  return new Promise(function (resolve) {
    // 模拟网络耗时，让进度条有得演
    setTimeout(function () {
      resolve({
        ok: true,
        engine: 'mock',
        courses: SAMPLE_COURSES
      })
    }, 1600)
  })
}

module.exports = {
  recognize: recognize
}
