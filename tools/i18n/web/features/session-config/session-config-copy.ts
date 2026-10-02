/**
 * session-config-copy — round 3 judge #8 (WEB-UX-ROUND3-INTEGRATION-4600):
 * the pane's GENERATED strings (dispositionNotice messages, strip words,
 * external-change notice) translated for the LOADED catalog. zh.ts belongs to
 * another owner; ui-text merges this table in. English source key → zh value.
 * Values carry no protocol vocabulary; server prose is interpolated, never
 * translated, so placeholder parity is the only structural constraint.
 */
export const SESSION_CONFIG_ZH_COPY: Readonly<Record<string, string>> =
  Object.freeze({
    "Saved. Your next message uses {value0}":
      "已保存。你的下一条消息将使用 {value0}",
    "Saved. The model is not active yet": "已保存。该模型尚未生效",
    "Saved. The model is not active yet ({value0})":
      "已保存。该模型尚未生效（{value0}）",
    "Saved. The server keeps running {value0} until it restarts":
      "已保存。服务器在重启前将继续运行 {value0}",
    "Saved, but not usable right now: {value0}":
      "已保存，但暂时无法使用：{value0}",
    "Already selected": "已选择此模型",
    "Couldn't save: {value0}": "无法保存：{value0}",
    Saved: "已保存",
    "the new model": "新模型",
    "the previous model": "之前的模型",
    "the runtime could not start": "运行时无法启动",
    "the server refused the change": "服务器拒绝了此更改",
    "The selection changed in another tab or app":
      "选择已在其他标签页或应用中更改",
  });
