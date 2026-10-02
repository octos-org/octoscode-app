/**
 * fleet-copy — the Fleet view's zh catalog (design WEB-UX-DESIGN-4000 §4
 * copy rules + §8 "both language catalogs carry every string"; grant
 * GLM-WEB-UX2-FLEET-VIEW-4010).
 *
 * A NEW file per the brief: `features/preferences/zh.ts` belongs to another
 * owner. English is the source key (the t() convention); this file carries
 * ONLY the fleet additions. No protocol vocabulary appears in any value
 * (lane → 模型, slug → 同侪/对等节点 name, operation ids hidden) — the catalog
 * test rejects seat/epoch/lane/slug/fence/operation id/binding.
 */

/** English source key → Simplified Chinese copy. */
export const FLEET_ZH_COPY: Readonly<Record<string, string>> = Object.freeze({
  Fleet: "舰队",
  "Back to Fleet": "返回舰队",
  "No peers yet": "还没有同侪",
  "Start a peer": "启动一个同侪",
  Model: "模型",
  Brief: "任务简报",
  Session: "会话",
  Start: "启动",
  "Starting…": "正在启动…",
  "Couldn't start: {value0}": "无法启动：{value0}",
  "Settings › Providers": "设置 › 提供商",
  Approve: "批准",
  Deny: "拒绝",
  Steer: "引导",
  "Steer {value0}": "引导 {value0}",
  "Approve for {value0}": "为 {value0} 批准",
  "Deny for {value0}": "为 {value0} 拒绝",
  "Stop {value0}": "停止 {value0}",
  "Only while the peer is running": "仅在同侪运行时可用",
  "Fleet live region": "舰队实时区域",
  Stop: "停止",
  Send: "发送",
  Advanced: "高级",
  Peers: "同侪",
  Requested: "已请求",
  Starting: "正在启动",
  "Still starting…": "仍在启动…",
  Working: "工作中",
  "Waiting for your approval": "等待你的批准",
  "Waiting for your answer": "等待你的回答",
  Finished: "已完成",
  Stopped: "已停止",
  Failed: "已失败",
  "Outcome unknown": "结果未知",
  Sent: "已发送",
  "Stop requested": "已请求停止",
  "Already handled": "已被处理",
  "Peer started a new turn": "同侪开始了新的回合",
  "Take control of {value0} to do this": "需要先取得 {value0} 的控制权",
  "Couldn't restore this peer": "无法恢复这个同侪",
  Retry: "重试",
  "Restoring…": "正在恢复…",
  "Peer started {value0}": "同侪启动于 {value0}",
  "Finished ({value0})": "已完成（{value0}）",
  "Loading peers for {value0} sessions": "正在为 {value0} 个会话加载同侪",
  "This server does not support remote control of peers":
    "此服务器不支持远程控制同侪",
  "This server does not support starting peers": "此服务器不支持启动同侪",
  "Peer controls are not ready": "同侪控制尚未就绪",
  "No peer models are configured — add one under Settings › Providers":
    "尚未配置同侪模型 — 请在 设置 › 提供商 中添加",
  "Open a project first": "请先打开一个项目",
  "Loading models…": "正在加载模型…",
  "Not sure it started — Retry resends the same request.":
    "不确定是否已启动 — 重试会重发同一请求。",
  Dismiss: "关闭",
  "Only while waiting for approval": "仅在等待批准时可用",
  "Only while waiting for your answer": "仅在等待你的回答时可用",
  "Only while working": "仅在工作中可用",
  "Enter steering text": "输入引导文字",
  "{value0} is waiting for your approval": "{value0} 正在等待你的批准",
  "{value0} is waiting for your answer": "{value0} 正在等待你的回答",
  "{value0} finished": "{value0} 已完成",
  "{value0} stopped": "{value0} 已停止",
  "{value0} failed": "{value0} 已失败",
  "Peer {value0}": "同侪 {value0}",
  "Session peers": "会话同侪",
  "Goal {value0}": "目标 {value0}",
});

/** The bounded key list, so coverage tests never drift from the catalog. */
export function fleetCopyKeys(): string[] {
  return Object.keys(FLEET_ZH_COPY);
}
