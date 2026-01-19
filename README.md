迭代二（V2）工程任务拆解清单（你可以直接开发）
🧩 模块一：便签身份与生命周期（必须最先做）

任务 2.1｜重构 noteId 生成规则

 创建便签时由 Rust 生成 uuid

 noteId 永远来自 index.json

 窗口 label 只是映射，不是身份

任务 2.2｜补齐 index.json 的「时间字段」

每个 node 至少包含：

{
  "id": "uuid",
  "createdAt": "...",
  "lastActiveAt": "...",
  "expireAt": "...",
  "archived": false
}

🧩 模块二：7 天消失机制（这是 FadeNote 的灵魂）

任务 2.3｜定义唯一判断规则

7 天 = now - lastActiveAt > 7d

❌ 不是 based on 文件夹

❌ 不是 based on createdAt

任务 2.4｜启动时执行一次「过期处理」

 App 启动

 扫描 index.json

 标记 expired → archived

 UI 永不再加载 archived

🧩 模块三：保存策略降级（反“笔记化”）

任务 2.5｜保存从「输入驱动」改为「行为驱动」

建议规则：

输入中 → 不立刻持久化

以下行为才更新 lastActiveAt：

窗口获得焦点

内容发生变化并 idle ≥ 3s

窗口被拖拽

🧩 模块四：隐藏技术结构（产品层清理）

任务 2.6｜Front Matter 仅存在于底层

 JS 层永远只处理纯文本

 Rust 层负责拼装 / 解析 md

 JS 不再感知 ---

🧩 模块五：明确「现在不做什么」（同样重要）

V2 明确不做：

❌ 搜索

❌ 标签

❌ 长期归档查看

❌ 文件夹浏览

FadeNote 不是一个“我要找回以前写的东西”的产品