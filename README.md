# FadeNote · Index.json V2 README（冻结版）

> 本文档是 **FadeNote V2 的宪法级实现规范**。
>
> 所有实现 **必须以本 README 为唯一事实来源**，禁止自行推断、补充或优化设计。

---

## 一、V2 目标与边界

### 目标

* 稳定 index.json 为 **唯一真相源**
* 不扫描磁盘、不遍历 md
* 自动生命周期，不打扰用户
* 为 V3 留出扩展空间

### 明确不做（V2 禁止）

* ❌ 搜索
* ❌ 标签 / 文件夹管理
* ❌ 手动整理
* ❌ 批量操作
* ❌ 从 md 反向构建 UI

---

## 二、index.json 顶层结构（V2 冻结）

```json
{
  "version": 2,
  "app": { ... },
  "notes": [ ... ]
}
```

---

## 三、app 字段定义（只读元信息）

```json
"app": {
  "name": "FadeNote",
  "createdAt": "",
  "rebuildAt": ""
}
```

### 字段规则

* `createdAt`

  * 首次生成 index.json 时写入
  * 永不修改

* `rebuildAt`

  * **仅在重建 index.json 时写入**
  * 普通启动 / 更新禁止写入

### 铁律

> **任何业务逻辑禁止依赖 app 字段**

该字段仅用于调试、诊断、开发者判断。

---

## 四、note 对象（核心数据结构）

### 新增字段：cachedPreview

```json
"cachedPreview": "string | null"
```

### cachedPreview 宪法级规则

> cachedPreview 是 **UI 缓存，不是数据源**。

#### 唯一允许的写入时机

```text
用户编辑
→ 内容发生实质性变化
→ 从编辑器内存解析首行
→ 写入 index.json.cachedPreview
```

#### 严格限制

* 只在 **编辑态**
* 只从 **内存** 读取
* 只在 **内容变化** 时写入
* ❌ 禁止读取 md
* ❌ 禁止启动时补全
* ❌ 禁止归档视图生成

---

## 五、启动时强制流程（宪法级）

```text
1. 读取 index.json
2. 若不存在 / 解析失败 → rebuild
3. normalizeIndex（修正非法状态）
4. 过期判断 → 自动归档
5. UI 恢复（仅 window != null）
```

### normalizeIndex 要求

* archived=true 的 note 不得出现在桌面
* window=null 的 note 不创建窗口
* 非法字段值必须被修正，而不是报错

---

## 六、index 重建（Rebuild）宪法裁定

### 触发条件

* index.json 不存在
* index.json 无法解析

### 重建结果（强制）

```text
所有 note = active
所有 window = null
生命周期重新开始
```

### 行为要求

* 不弹窗
* 不打扰用户
* 静默完成

---

## 七、系统托盘（Tray）定义

```text
FadeNote
────────────
New Note
Archive
────────────
Settings
Quit
```

### 规则

* Archive 不与 New Note 相邻
* Archive 是“模式切换”，不是操作
* 桌面便签禁止任何归档入口

---

## 八、归档窗口（Archive View）

### 定位

> 归档视图 = **只读的时间回溯列表**

不是管理器，不是第二工作区。

### 展示内容（每条仅 3 个信息）

* cachedPreview（或占位文案）
* lastActiveAt
* 隐含归档状态

```text
if cachedPreview exists:
  show cachedPreview
else:
  show "(Archived note)"
```

### 禁止行为

* ❌ 编辑
* ❌ 删除
* ❌ 批量操作
* ❌ 搜索
* ❌ 排序切换

---

## 九、归档视图交互铁律

### 默认状态

* 单击：无反应
* Hover：轻微高亮
* 只读

### 唯一主动操作：打开

#### 双击 / Enter

```text
1. 打开 md 内容
2. 进入编辑态
3. 内容发生实质性变化
4. archived = false
5. lastActiveAt = now
6. expireAt = now + 7d
7. 出现在桌面
```

> 系统不显示“恢复 / 复活”概念，一切自动完成。

---

## 十、V2 开发任务清单（按优先级）

### P0（必须完成）

* [ ] index.json V2 结构升级
* [ ] app 字段写入规则实现
* [ ] cachedPreview 写入规则实现
* [ ] 启动强制流程实现
* [ ] index 重建逻辑

### P1（核心体验）

* [ ] 系统 Tray 菜单
* [ ] Archive 窗口创建
* [ ] archived note 列表渲染
* [ ] 双击打开 → 生命周期迁移

### P2（稳定性）

* [ ] normalizeIndex 完整覆盖
* [ ] 非法状态容错
* [ ] index 写入原子性保证

---

## 十一、最终宪法声明

> **index.json 是 FadeNote 的唯一真相**
> **UI 永远不反向解释文件系统**
> **FadeNote 不鼓励管理，只允许发生**

---

（本 README 即 V2 冻结规范，非升级版本不得修改）
