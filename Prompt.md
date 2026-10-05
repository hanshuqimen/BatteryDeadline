# BatteryDeadline — Engineering Specification & Codex Implementation Prompt

你现在是一名资深 Windows 系统软件工程师、Rust 工程师、桌面应用架构师和产品工程师。

请你从零开始设计并实现一个可实际发布到 GitHub 的开源项目：

# BatteryDeadline

Tagline:

**Make your laptop last until you need it.**

或者：

**Cruise control for laptop battery life.**

BatteryDeadline 不是普通的“省电模式”。

用户不是告诉软件：

> 把 CPU 限制到 60%。

也不是：

> 开启节能模式。

用户告诉软件：

> **“现在是 13:40，我需要这台电脑至少撑到 18:30。”**

BatteryDeadline 应根据：

- 当前剩余电量；
- 当前剩余电池容量；
- 实时放电功率；
- 用户指定的截止时间；
- 用户预留的最低电量；
- 当前设备能力；
- 用户允许调整的项目；

计算：

> 为了撑到 deadline，这台电脑接下来允许的平均功耗是多少？

然后以一个缓慢、稳定、可恢复的反馈控制器动态调整系统，使电脑尽可能达到目标，同时尽量减少对用户体验的影响。

核心思想不是：

**Maximum battery life.**

而是：

**Enough battery life.**

---

# 0. 最重要的开发原则

这是一个真实系统工具。

不是 Demo。

不是 UI prototype。

不是“先把界面做出来，以后补系统调用”。

从第一版开始，架构必须围绕：

**Telemetry → Estimation → Decision → Actuation → Observation → Correction**

这个闭环设计。

任何涉及 Windows 系统状态修改的功能必须：

1. 可检测是否支持；
2. 修改前记录原值；
3. 修改后可验证；
4. 用户停止 BatteryDeadline 时恢复；
5. 插入电源时恢复；
6. 程序正常退出时恢复；
7. 程序异常退出后，下次启动能够检测并恢复。

绝不能留下：

- 被永久改变的刷新率；
- 被永久改变的亮度；
- 被污染的 Windows 电源计划；
- 永久被 throttling 的进程；
- BatteryDeadline 崩溃后无法恢复的系统状态。

任何优化动作都必须遵守：

> **reversible by design**

---

# 1. 第一版平台

只支持：

**Windows 11 x64**

优先支持：

**Windows 11 24H2 及以上**

暂时不要：

- Linux；
- macOS；
- ARM64；
- 手机；
- Web；
- kernel driver；
- 厂商专用 BIOS 控制；
- NVIDIA NVAPI；
- AMD ADLX；
- Intel 专用 SDK。

架构应允许以后添加平台和厂商插件，但 MVP 不实现。

---

# 2. 技术栈

桌面框架：

**Tauri 2**

后端：

**Rust**

前端：

**React + TypeScript + Vite**

UI 可以使用：

**Tailwind CSS**

允许使用轻量组件库，但不要把项目做成一个巨大的 UI dependency tree。

持久化：

**SQLite**

Rust 可以使用：

- rusqlite；
- serde；
- serde_json；
- chrono；
- tracing；
- anyhow / thiserror；
- windows crate；

以及确实必要的成熟依赖。

不要因为“方便”引入 Electron。

BatteryDeadline 应保持：

- 小；
- 快；
- 启动迅速；
- 后台资源占用低；
- 内存占用合理。

它本身不能成为耗电大户。

---

# 3. Repository Structure

请设计清晰的 workspace。

推荐：

```text
BatteryDeadline/
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs
│  │  ├─ app.rs
│  │  │
│  │  ├─ telemetry/
│  │  │  ├─ mod.rs
│  │  │  ├─ battery.rs
│  │  │  ├─ system.rs
│  │  │  └─ process.rs
│  │  │
│  │  ├─ estimator/
│  │  │  ├─ mod.rs
│  │  │  ├─ discharge.rs
│  │  │  └─ prediction.rs
│  │  │
│  │  ├─ controller/
│  │  │  ├─ mod.rs
│  │  │  ├─ budget.rs
│  │  │  ├─ policy.rs
│  │  │  └─ state_machine.rs
│  │  │
│  │  ├─ actuators/
│  │  │  ├─ mod.rs
│  │  │  ├─ brightness.rs
│  │  │  ├─ refresh_rate.rs
│  │  │  ├─ power_plan.rs
│  │  │  └─ eco_qos.rs
│  │  │
│  │  ├─ recovery/
│  │  │  ├─ mod.rs
│  │  │  └─ journal.rs
│  │  │
│  │  ├─ storage/
│  │  │  ├─ mod.rs
│  │  │  └─ database.rs
│  │  │
│  │  ├─ platform/
│  │  │  ├─ mod.rs
│  │  │  ├─ traits.rs
│  │  │  └─ windows/
│  │  │
│  │  └─ simulator/
│  │     ├─ mod.rs
│  │     └─ fake_hardware.rs
│  │
│  └─ Cargo.toml
│
├─ src/
│  ├─ components/
│  ├─ pages/
│  ├─ hooks/
│  ├─ stores/
│  ├─ types/
│  └─ App.tsx
│
├─ tests/
├─ docs/
├─ .github/workflows/
├─ README.md
├─ CONTRIBUTING.md
├─ LICENSE
└─ SECURITY.md
```

不要求机械遵守此结构。

如果你有更合理的组织方式，可以调整。

但：

**telemetry、estimator、controller、actuator、recovery 必须彼此解耦。**

---

# 4. Windows Battery Telemetry

第一优先级是得到可信的电池数据。

实现两级 telemetry。

## Level A：通用电源状态

使用官方 Windows API 获取：

- AC / Battery 状态；
- Battery Percentage；
- charging/discharging；
- 系统报告的 remaining lifetime，如果存在。

可使用：

`GetSystemPowerStatus`

但不要只依赖这个 API。

---

# 5. Level B：详细 Battery Device 数据

枚举 Battery Class Devices。

通过官方 Battery Device IOCTL 获取：

- BatteryTag；
- BATTERY_INFORMATION；
- FullChargedCapacity；
- DesignedCapacity；
- BATTERY_STATUS；
- current Capacity；
- Voltage；
- Rate。

优先使用：

`IOCTL_BATTERY_QUERY_TAG`

`IOCTL_BATTERY_QUERY_INFORMATION`

`IOCTL_BATTERY_QUERY_STATUS`

关键：

BATTERY_STATUS 中，如果硬件提供可靠数据：

```text
Capacity ≈ remaining energy in mWh
Rate     ≈ charge/discharge rate in mW
```

放电时 Rate 通常是负数。

内部统一成：

```text
discharge_power_w > 0
```

例如 Windows 返回：

```text
Rate = -8400 mW
```

内部：

```text
discharge_power_w = 8.4
```

如果设备返回：

- unknown capacity；
- unknown rate；
- relative capacity；
- 不支持 battery IOCTL；

必须自动 fallback。

绝不能 crash。

---

# 6. Telemetry Fallback

如果硬件没有实时功率 Rate：

使用时间窗口推断。

例如记录：

```text
t0 capacity
t1 capacity
```

如果容量单位可信：

```text
P ≈ ΔEnergy / ΔTime
```

如果只能获取百分比：

允许使用百分比斜率预测：

```text
percentage / hour
```

但 UI 必须降低 Confidence。

定义：

```rust
enum TelemetryQuality {
    Excellent,
    Good,
    Estimated,
    Poor,
}
```

UI 中必须清楚显示：

```text
Prediction confidence: High
```

或者：

```text
Prediction confidence: Low
Battery does not report real-time power usage.
```

不要伪装精确。

---

# 7. 核心数学模型

用户输入：

```text
deadline = 18:30
reserve = 10%
```

当前时间：

```text
14:30
```

剩余：

```text
T = 4 hours
```

假设：

```text
CurrentCapacity = 42 Wh
FullChargeCapacity = 60 Wh
Reserve = 6 Wh
```

可使用电量：

```text
E_available = 42 - 6
            = 36 Wh
```

最大允许平均功耗：

```text
P_budget = E_available / T
         = 9 W
```

当前稳定放电：

```text
P_current = 13.4 W
```

所以：

```text
need_to_save ≈ 4.4 W
```

UI：

```text
Current draw
13.4 W

Required average
≤ 9.0 W

Need to save
4.4 W
```

如果：

```text
P_current < P_budget
```

则有 surplus。

例如：

```text
Power headroom
+2.8 W
```

此时 controller 应允许逐渐恢复一些用户体验，而不是永远保持最激进省电状态。

---

# 8. Prediction Engine

不要用瞬时功耗直接预测。

功耗会剧烈波动。

实现：

- short rolling median；
- long EWMA；
- outlier rejection；
- minimum sample window；
- variance estimation。

建议：

```text
raw samples: 1 sample / second

short window:
15–30 seconds

long estimator:
60–180 seconds
```

如果电脑刚从：

```text
10 W
```

突然变成：

```text
35 W
```

不要立即把屏幕砍到 20%。

必须先判断变化是否持续。

预测输出至少包含：

```rust
struct Prediction {
    estimated_power_w: f64,
    power_budget_w: Option<f64>,
    predicted_remaining_minutes: Option<f64>,
    predicted_depletion_time: Option<DateTime>,
    deadline_margin_minutes: Option<f64>,
    confidence: Confidence,
}
```

---

# 9. Margin

用户真正关心的是：

```text
Will I make it?
```

定义：

```text
margin = predicted_depletion_time - deadline
```

UI：

安全：

```text
Expected to last until 18:47

Deadline:
18:30

Margin:
+17 min
```

危险：

```text
Expected to last until 17:52

Deadline:
18:30

Shortfall:
38 min
```

这比单纯显示“预计续航 4h17m”更重要。

---

# 10. Controller State Machine

实现明确状态机：

```text
IDLE

MONITORING

ADJUSTING

STABLE

AT_RISK

CRITICAL

PAUSED_AC

RESTORING

ERROR
```

例如：

```text
IDLE
 ↓ Start
MONITORING
 ↓ insufficient margin
ADJUSTING
 ↓ acceptable margin
STABLE
 ↓ workload increases
AT_RISK
 ↓ controller acts
ADJUSTING
```

插入电源：

```text
ANY ACTIVE STATE
 ↓ AC detected
RESTORING
 ↓
PAUSED_AC
```

用户 Stop：

```text
ANY ACTIVE STATE
 ↓
RESTORING
 ↓
IDLE
```

---

# 11. 不允许控制器“抽风”

必须设计 hysteresis。

例如目标：

```text
desired safety margin = +15 min
```

不要：

```text
+14 min → 降性能
+16 min → 升性能
+14 min → 降性能
...
```

定义 deadband，例如：

```text
lower bound: +8 min
upper bound: +22 min
```

margin 在这个范围内：

**不要动。**

每个 actuator 还必须有：

- cooldown；
- minimum dwell time；
- change rate limit。

例如：

刷新率修改之后至少：

```text
60 seconds
```

才能再次调整。

亮度调整后至少：

```text
20 seconds
```

再判断效果。

控制器一次只改变一个主要变量，然后观察效果。

这样以后可以计算：

```text
brightness -10%
observed saving ≈ 0.7 W
```

而不是同时修改五项之后完全不知道谁有效。

---

# 12. Actuator Priority

MVP 不追求控制所有硬件。

优先实现最安全、最通用、最容易恢复的 actuator。

默认优先级：

```text
1 Brightness
2 Display refresh rate
3 Windows power plan / processor policy
4 Optional EcoQoS background processes
```

---

# 13. Brightness Controller

优先控制内置笔记本屏幕。

使用 Windows WMI monitor brightness API。

必须：

- 查询当前 brightness；
- 保存原值；
- 设置 brightness；
- 验证结果；
- 支持恢复。

用户设置：

```text
Minimum brightness:
40%
```

BatteryDeadline 永远不能低于它。

例如：

```text
Current 72%

Controller wants:
58%

Allowed:
yes
```

如果想：

```text
32%
```

但 minimum：

```text
40%
```

实际只能：

```text
40%
```

用户手动修改亮度时：

不要和用户抢控制权。

检测用户 override 后：

更新 baseline 或暂停这个 actuator 一段时间。

---

# 14. Refresh Rate Controller

枚举当前显示器支持的合法模式。

禁止生成不存在的 refresh rate。

例如设备支持：

```text
165
120
60
```

才能切：

```text
165 → 120 → 60
```

不能自己编：

```text
165 → 90
```

除非系统枚举中确实存在 90 Hz。

通过 Windows documented display API 修改。

MVP：

只改变：

```text
dmDisplayFrequency
```

不要改变：

- resolution；
- HDR；
- color depth；
- scaling；
- orientation。

修改前保存：

```text
display_id
original_frequency
```

程序退出必须恢复。

---

# 15. Power Plan Controller

绝不能直接污染用户原本的 power plan。

正确策略：

读取当前 active scheme：

```text
OriginalPlan
```

复制：

```text
OriginalPlan
      ↓ clone
BatteryDeadline Temporary
```

所有修改作用于：

```text
BatteryDeadline Temporary
```

程序停止后：

```text
restore OriginalPlan
```

如果安全：

删除 temporary plan。

MVP 可以只控制 DC battery 下的少量 processor settings。

例如：

```text
Maximum processor state
```

设计几个档：

```text
100%
90%
80%
70%
```

不要一开始实现几十项 hidden Windows power setting。

不要使用 undocumented Windows APIs。

不要依赖：

```text
powercfg command shell hacks
```

除非某一能力官方 API 无法可靠实现，并且经过明确封装。

优先直接调用官方 PowrProf API。

所有 GUID 必须来自 Windows SDK / 官方定义或清晰集中定义并附注来源。

不要在业务逻辑中散落 magic GUID。

---

# 16. EcoQoS

这是实验功能。

MVP UI 默认：

```text
Background process optimization
OFF
```

用户手动打开才使用。

只允许对：

- 当前用户拥有；
- 非系统；
- 非 protected；
- 非 foreground；
- 非 BatteryDeadline；
- 非明确排除；

的后台进程尝试 EcoQoS。

使用官方：

`SetProcessInformation`

`ProcessPowerThrottling`

`PROCESS_POWER_THROTTLING_EXECUTION_SPEED`

不要：

- suspend 进程；
- kill 进程；
- 修改 affinity；
- 修改 priority class；
- throttle 当前前台程序；
- throttle 游戏；
- throttle 音频实时程序；
- throttle shell/system process。

提供 blacklist：

```text
explorer.exe
dwm.exe
audiodg.exe
csrss.exe
winlogon.exe
...
```

但不要只依赖 process name。

必须同时做安全检查。

如果权限不足：

跳过。

不要请求管理员权限只是为了 throttle 一个进程。

---

# 17. User Comfort Constraints

用户可以定义：

```text
Minimum brightness       45%
Minimum refresh rate     60Hz

Allow CPU power limits   Yes
Allow background EcoQoS  No
```

还提供三个 preset：

```text
Comfort
Balanced
Aggressive
```

但 preset 实际映射到明确配置。

不要放“神秘 AI 优化”。

例如：

Comfort：

```text
brightness >= 60%
refresh >= 60Hz
CPU >= 90%
EcoQoS off
```

Balanced：

```text
brightness >= 45%
refresh >= 60Hz
CPU >= 80%
```

Aggressive：

```text
brightness >= 30%
refresh >= lowest safe supported rate
CPU >= 65%
optional EcoQoS
```

---

# 18. UI 首页

首页必须一眼回答四件事：

```text
Battery
Deadline
Will I make it?
What is BatteryDeadline doing?
```

推荐：

```text
┌──────────────────────────────────┐
│ BatteryDeadline                  │
│                                  │
│ 63%                              │
│                                  │
│ Need this laptop until           │
│                                  │
│        18:30                     │
│                                  │
│ Predicted                        │
│ 18:47                            │
│                                  │
│ ✓ On track                       │
│ +17 min margin                   │
│                                  │
│ Current draw      8.4 W          │
│ Power budget      9.1 W          │
│                                  │
│ [ Stop ]                         │
└──────────────────────────────────┘
```

如果不够：

```text
⚠ At risk

Predicted:
17:51

Deadline:
18:30

Shortfall:
39 min

BatteryDeadline is adjusting your PC.
```

---

# 19. Activity Timeline

用户必须知道软件做了什么。

例如：

```text
13:41
Session started

13:43
Brightness
72% → 62%
Estimated saving: 0.6 W

13:46
Refresh rate
120Hz → 60Hz
Estimated saving: 0.8 W

13:51
Power budget reached

14:22
Workload increased
8.1 W → 12.8 W

14:23
Processor power limit
100% → 90%
```

每一项可以展开：

```text
Why?

Your predicted battery life fell
28 minutes short of the deadline.
```

---

# 20. 所有决策必须可解释

Controller 输出：

```rust
struct Decision {
    reason: DecisionReason,
    action: Action,
    before: Value,
    after: Value,
    expected_effect: Option<f64>,
}
```

不要只输出：

```text
Optimization applied.
```

而要：

```text
Reduced refresh rate because the laptop
was predicted to miss the deadline by 31 minutes.
```

---

# 21. Crash-Safe Recovery Journal

这是整个项目最重要的工程功能之一。

每次 actuator 修改之前：

先持久化：

```text
original state
desired new state
action pending
```

修改完成后：

```text
action applied
```

例如：

```json
{
  "session_id": "...",
  "actuator": "refresh_rate",
  "target": "DISPLAY1",
  "original": 120,
  "applied": 60,
  "restored": false
}
```

正常停止：

逐条恢复。

然后：

```text
restored = true
```

如果 BatteryDeadline crash。

下次启动检查：

```text
unrestored actions exist?
```

如果存在：

立即进入：

```text
RECOVERY MODE
```

UI：

```text
BatteryDeadline did not exit cleanly.

Some system settings may still be active.

[ Restore now ]
```

默认自动安全恢复。

---

# 22. AC Behavior

只要检测到 AC：

立即停止主动省电调节。

执行：

```text
RESTORING
```

恢复 BatteryDeadline 修改过的：

- brightness；
- refresh rate；
- temporary power plan；
- EcoQoS。

然后：

```text
PAUSED — Plugged in
```

如果拔掉电源：

不要立即自动重新开始，除非用户开启：

```text
Resume automatically on battery
```

---

# 23. Deadline Behavior

到达 deadline 时，不应继续“为了无限续航”限制电脑。

deadline 达到：

```text
Goal reached 🎉
```

然后询问/根据设置：

```text
Restore performance now
```

默认恢复。

因为产品目标是：

> make it until X

不是：

> permanently cripple my laptop.

---

# 24. Battery Reserve

默认：

```text
10%
```

允许：

```text
5–30%
```

不要计划把电脑正好榨到 0%。

UI：

```text
Reserve:
10%

BatteryDeadline targets reaching
18:30 with roughly 10% remaining.
```

如果用户 deadline 本身不可能实现：

例如：

```text
battery 8%
deadline 5 hours away
reserve 10%
```

直接告诉用户：

```text
This deadline cannot be met with the
current reserve setting.
```

不要做荒谬调整。

---

# 25. Feasibility

定义状态：

```text
Easy
Possible
Tight
Unlikely
Impossible
```

示例：

```text
Current:
14.2 W

Required:
≤ 5.1 W

Observed minimum during this session:
8.7 W
```

因此：

```text
Unlikely
```

UI 应诚实：

```text
BatteryDeadline probably cannot reach 19:00
without changing how the laptop is being used.
```

而不是承诺一定做到。

---

# 26. Learning Actuator Effects

每次调整 actuator：

记录：

```text
before_power
after_power
environment
change
```

例如：

```text
Brightness
70 → 60

Observed:
-0.58 W
```

以后在同一设备可以形成：

```text
Effect model
```

例如：

```text
brightness 10% ≈ 0.55 W
120Hz → 60Hz ≈ 0.82 W
CPU 100 → 90 ≈ 1.31 W
```

Controller 优先选择：

```text
energy_saved / user_discomfort
```

比较好的动作。

第一版可以用简单 moving average。

不要上 ML。

---

# 27. Control Cost

给每种 actuator 一个 discomfort cost。

例如：

```text
brightness -10%
cost = 1

120 → 60Hz
cost = 2

CPU 100 → 90
cost = 2

CPU 90 → 70
cost = 5
```

Controller 目标变成：

```text
Minimize total discomfort
subject to:
predicted battery >= deadline + safety margin
```

不需要真的上复杂 optimizer。

MVP 使用 greedy controller 即可：

```text
choose lowest-cost action expected
to close the current power deficit
```

---

# 28. Simulation Mode

必须从第一天实现。

这是开发效率的关键。

提供：

```bash
battery-deadline --simulate
```

或者 debug build 中：

```text
Simulation Mode
```

Fake battery：

```text
capacity = 40 Wh
full = 60 Wh
rate = 12 W
```

可以模拟：

```text
workload spike
AC plugged
battery rate unavailable
display unsupported
actuator failure
deadline impossible
program crash recovery
```

Controller 所有核心逻辑必须能在没有真实 Windows laptop 的 CI 中测试。

平台接口必须抽象：

```rust
trait BatteryProvider
trait BrightnessController
trait DisplayController
trait PowerPlanController
trait ProcessPowerController
```

Production：

```text
WindowsBatteryProvider
```

Tests：

```text
FakeBatteryProvider
```

---

# 29. SQLite

至少保存：

```text
settings

sessions

telemetry_samples

actions

actuator_effects

recovery_journal
```

Telemetry 不需要永久保存每秒数据。

实现压缩/retention。

例如：

当前 session：

```text
1 second resolution
```

session 完成后：

聚合为：

```text
10s / 30s resolution
```

或者只保留最近 7 天高精度记录。

避免数据库无限膨胀。

---

# 30. Tray

BatteryDeadline 主要是后台工具。

实现 System Tray。

Tray tooltip：

```text
BatteryDeadline
63% · on track · +17 min
```

菜单：

```text
Open

Deadline: 18:30
Prediction: 18:47

Pause
Restore system settings
Quit
```

点击 Quit：

必须先 restore。

---

# 31. Onboarding

第一次启动执行 Capability Scan。

例如：

```text
Battery telemetry
✓ Real-time power rate

Brightness control
✓ Supported

Refresh rate control
✓ 60 / 120 Hz

Power plan control
✓ Supported

Background EcoQoS
✓ Available

Battery health
87%
```

如果某能力不支持：

```text
Brightness control
Unavailable on this display
```

继续运行。

不能因为一个 actuator 不支持而让整个 App 无法使用。

---

# 32. Privacy

BatteryDeadline 默认：

**100% local.**

不要：

- telemetry 上传；
- analytics；
- account；
- cloud；
- login；
- ads；
- remote API；
- LLM API。

README 明确：

```text
BatteryDeadline does not send your
battery or usage data anywhere.
```

以后即使增加匿名 telemetry，也必须 opt-in。

MVP 完全不做。

---

# 33. Security

不要：

- kernel driver；
- DLL injection；
- code injection；
- undocumented system hook；
- disable Windows Defender；
- manipulate thermal limits；
- BIOS calls；
- undocumented EC writes；
- arbitrary registry tweaking。

默认运行：

**standard user**

只有某项能力确实要求 elevation 时：

单独请求。

不能：

```text
Run BatteryDeadline as Administrator
```

作为默认要求。

---

# 34. 性能预算

BatteryDeadline 自己必须节能。

空闲后台目标：

```text
CPU < 0.5% average
```

最好远低于此。

不要：

```text
while true {
    poll_everything();
}
```

可以：

- battery telemetry 1 Hz；
- expensive process scan 5–10 s；
- UI closed 时降低更新频率；
- 尽量使用 Windows notifications。

前端不可见时不要以 60 FPS repaint dashboard。

---

# 35. Logging

使用结构化日志。

开发模式：

```text
DEBUG
```

release 默认：

```text
INFO / WARN
```

日志不能记录：

- 文件内容；
- 浏览数据；
- 用户文档；
- 敏感信息。

提供：

```text
Export diagnostics
```

生成 zip：

```text
battery-deadline.log
capabilities.json
version.json
```

方便 GitHub issue。

---

# 36. Unit Tests

重点测试 estimator 和 controller。

必须包含：

### Prediction

```text
40 Wh remaining
10 Wh reserve
3 h
→ budget 10 W
```

### Impossible deadline

```text
remaining usable energy <= 0
```

### High workload

```text
budget 8W
actual 14W
→ request reduction
```

### Surplus

```text
budget 12W
actual 8W
→ no action / cautiously relax
```

### Hysteresis

margin 在 deadband 内：

```text
no oscillation
```

### Missing Rate

自动 fallback。

### Plug AC

active → restore.

### Crash recovery

journal exists → restore.

---

# 37. Integration Tests

Simulation 模式完成：

```text
Start session
↓
consume too much
↓
controller reduces fake brightness
↓
prediction improves
↓
controller becomes stable
↓
AC plugged
↓
all fake actuators restored
```

另一个：

```text
controller changes display
↓
simulate crash
↓
restart app
↓
recovery journal detected
↓
restore display
```

必须自动化。

---

# 38. CI

GitHub Actions：

Windows runner。

至少执行：

```text
cargo fmt --check
cargo clippy
cargo test

npm install / pnpm install
typecheck
frontend tests
build
```

Release tag：

生成：

```text
Windows installer
portable executable if practical
checksums
```

不要一开始做复杂自动签名。

但架构不要阻碍以后 code signing。

---

# 39. UI 风格

不要做：

- 赛博朋克；
- RGB gamer UI；
- 一屏幕几十个传感器；
- 类似 MSI Afterburner；
- 工程师仪表盘。

这是普通用户也能理解的消费级工具。

风格：

- calm；
- minimal；
- clean；
- Windows 11 friendly。

核心语言：

```text
On track

17 minutes of margin

Using 1.8 W too much

Adjusting

Deadline unlikely

Restored
```

而不是：

```text
PID loop divergence coefficient
CPU P-State
QoS class
processor throttle index
```

这些只放 Advanced 页面。

---

# 40. MVP 页面

第一版只有：

```text
Dashboard
History
Settings
About
```

Dashboard：

当前 session。

History：

```text
Oct 5

Started     13:41
Deadline    18:30
Battery     63% → 14%
Result      Reached ✓

Estimated energy saved
6.8 Wh
```

不要假装能精确知道“节省 6.812 Wh”。

如果估算不可信：

标记：

```text
Estimated
```

Settings：

comfort limits。

---

# 41. MVP 范围

第一版必须完成：

```text
✓ Battery telemetry
✓ real-time power estimation
✓ deadline model
✓ battery reserve
✓ predicted depletion time
✓ confidence
✓ controller state machine
✓ brightness actuator
✓ refresh rate actuator
✓ reversible temporary power plan
✓ recovery journal
✓ simulator
✓ tray
✓ activity log
✓ history
```

EcoQoS 如果影响进度：

可以放到：

```text
v0.2
```

不要为了功能数量拖垮 MVP。

---

# 42. 明确不做

MVP 不做：

```text
AI
LLM
cloud sync
accounts
cross-device
manufacturer SDK
GPU undervolt
CPU undervolt
fan control
thermal control
BIOS
kernel driver
automatic app killing
network throttling
browser extension
Android
macOS
Linux
```

不要 scope creep。

---

# 43. Development Phases

按照以下顺序实际开发。

## Phase 0 — Repository bootstrap

完成：

- Rust/Tauri project；
- React frontend；
- formatting；
- lint；
- CI；
- logging；
- basic architecture。

确保：

```text
cargo test
frontend build
tauri build
```

工作。

---

## Phase 1 — Battery CLI Probe

在做漂亮 UI 之前先写一个内部 diagnostics command：

```bash
battery-deadline diagnostics battery
```

输出：

```text
AC: false
Percentage: 67%

Detailed battery:

Capacity:
42120 mWh

Full charge:
60200 mWh

Rate:
-8431 mW

Voltage:
11843 mV

Telemetry quality:
Excellent
```

如果真实 laptop 上这一层不能可靠工作：

先修。

不要继续做 UI。

---

## Phase 2 — Prediction Engine

实现纯 Rust：

```text
samples
↓
robust estimator
↓
power draw
↓
deadline budget
↓
prediction
```

100% 单元测试。

不依赖 Windows。

---

## Phase 3 — Simulator

让整个 controller 可以在假电池运行。

在进入 actuator 开发之前完成。

---

## Phase 4 — Brightness

实现：

```text
query
set
restore
```

完整测试错误处理。

---

## Phase 5 — Display refresh

实现：

```text
enumerate valid modes
query active
set supported mode
restore
```

禁止修改 resolution。

---

## Phase 6 — Temporary Power Plan

实现：

```text
clone
activate
modify DC values
restore original
delete temporary
```

实现 recovery。

---

## Phase 7 — Controller

连接：

```text
telemetry
prediction
policy
actuators
```

首先只运行：

```text
simulation
```

稳定后再接真实系统 actuator。

---

## Phase 8 — UI

在后台逻辑真正工作以后再完成 UI。

---

## Phase 9 — Recovery hardening

模拟各种错误：

```text
force kill
actuator fails
display disappears
AC plugs during action
battery device changes
sleep
resume
hibernate
```

确保系统设置不会卡死。

---

## Phase 10 — Packaging

最终做到：

用户从 GitHub Release 下载 Installer：

安装 → 打开 → 选时间 → Start。

---

# 44. README

README 第一屏要非常简单：

```text
BatteryDeadline

Make your laptop last until you need it.

63% battery.

You need your laptop until 6:30 PM.

BatteryDeadline calculates how much power
you can afford to use and gently adjusts
your PC to stay on schedule.
```

然后放 UI screenshot。

再解释：

```text
Not a battery saver.

A battery deadline controller.
```

README 包含：

- Why；
- How it works；
- Screenshots；
- Supported systems；
- Safety；
- Privacy；
- Limitations；
- Build instructions；
- Contributing。

---

# 45. Product Terminology

代码内部可以叫：

```text
power budget
controller
actuator
telemetry
margin
```

用户界面优先叫：

```text
Need until

Expected until

On track

Minutes ahead

Minutes short

Current usage

Adjusting

Restore
```

不要让普通用户学习控制理论。

---

# 46. Initial Product Defaults

第一次打开：

```text
Reserve:
10%

Mode:
Balanced

Minimum brightness:
45%

Minimum refresh rate:
60 Hz

CPU optimization:
On

Background process optimization:
Off

Restore when plugged in:
On

Restore at deadline:
On
```

不要未经许可做 aggressive changes。

---

# 47. Error Handling

任何 Windows API 失败都必须变成：

```rust
Result<T, BatteryDeadlineError>
```

不要：

```rust
unwrap()
expect()
```

出现在 production platform code 中。

UI 不能显示：

```text
HRESULT 0x80041010
```

而应显示：

```text
Brightness control is not available on this display.
```

日志里保留原始 error code。

---

# 48. Capability-Based Design

绝不能假设：

```text
every laptop supports brightness
every battery reports power
every monitor has multiple refresh rates
every machine exposes processor settings
```

每个 actuator：

```rust
trait Actuator {
    fn capabilities(...)
    fn snapshot(...)
    fn apply(...)
    fn restore(...)
}
```

Controller 根据 Capability Registry 动态工作。

某机器可能只有：

```text
Battery telemetry
Brightness
```

BatteryDeadline 仍然有价值。

---

# 49. Sleep / Resume

检测 suspend/resume。

Suspend 前：

持久化 recovery journal。

Resume：

重新读取：

- AC state；
- battery tag；
- battery capacity；
- display state；
- power plan；

不要认为睡眠前硬件状态仍然有效。

预测器：

resume 后清空短期功耗窗口，重新 warm up。

---

# 50. User Overrides

这是系统工具必须做好的细节。

如果 BatteryDeadline 设置：

```text
brightness = 55%
```

用户随后手动调到：

```text
80%
```

不要 1 秒以后：

```text
BatteryDeadline: nope → 55%
```

这会让用户愤怒。

识别 manual override。

然后：

```text
User changed brightness.
Brightness control paused for 10 minutes.
```

或者：

```text
Respect user override
```

刷新率同理。

**用户永远优先。**

---

# 51. Advanced Debug Panel

开发模式或 Advanced 页面可以展示：

```text
Battery rate      8.43 W

EWMA              8.72 W

Power budget      9.14 W

Margin            +17.2 min

Controller state  STABLE

Telemetry quality Excellent

Active actuators
brightness        62%
refresh           60 Hz
CPU               90%
```

方便 GitHub issue。

---

# 52. 项目的核心验收场景

最终 MVP 必须完成下面这个真实体验：

用户笔记本：

```text
Battery:
65%

Time:
14:00

Current power:
13 W

Predicted empty:
17:05
```

用户：

```text
Need until:
18:00

Reserve:
10%
```

BatteryDeadline：

```text
Required average:
8.3 W

Current:
13 W

Shortfall:
55 min
```

随后按最低体验损失逐步：

```text
brightness
75 → 60

refresh
120 → 60

CPU
100 → 90
```

功耗降为：

```text
8.1 W
```

预测：

```text
Expected:
18:14

Margin:
+14 min

✓ On track
```

然后用户打开高负载程序：

```text
power:
15 W
```

控制器检测持续负载提高：

```text
margin falls
```

再次进行有限调整。

用户插电：

```text
BatteryDeadline immediately stops controlling.
```

恢复：

```text
brightness → original/user value
refresh → original
power plan → original
```

这整个流程必须可靠。

---

# 53. 不要过度承诺

BatteryDeadline 不能承诺：

```text
Guaranteed to last until 18:30
```

只能：

```text
Currently on track to last until 18:30.
```

因为用户可能突然：

- 打游戏；
- 编译；
- 开 Blender；
- 视频会议；
- 调到最大亮度。

产品语义必须诚实。

---

# 54. Code Quality

整个项目必须达到可以公开 GitHub 的标准：

- idiomatic Rust；
- clear module boundaries；
- documentation；
- tests；
- no giant 3000-line file；
- no unexplained unsafe blocks；
- minimal unsafe；
- unsafe 必须封装在小型 Windows API layer；
- every unsafe block 写 SAFETY comment；
- TypeScript strict；
- no `any` unless unavoidable；
- no dead code；
- no placeholder button；
- no fake graph；
- no hardcoded UI sensor values。

如果功能尚未实现：

不要做一个可以点击但没有作用的按钮。

宁愿不显示。

---

# 55. Codex 工作方式

你不是只负责生成代码片段。

你负责将这个项目推进到真正可运行状态。

开始之前：

1. 检查当前仓库。
2. 如果为空，初始化完整项目。
3. 创建合理架构。
4. 先完成 Phase 0。
5. 编译。
6. 测试。
7. 修复。
8. 再进入下一阶段。

每完成一个重要阶段：

实际运行：

```text
cargo fmt
cargo clippy
cargo test
frontend typecheck
frontend test
build
```

发现错误直接修。

不要把明显的问题留给用户。

不要每一步都询问用户。

如果规范中存在非关键歧义：

选择工程上更合理、保守、安全的方案，并在文档记录。

只有存在真正无法继续的外部依赖时才停下来。

---

# 56. 首轮任务

现在开始执行。

第一轮目标不是一次写完整个项目。

第一轮必须完成：

```text
Phase 0
+
Phase 1
+
Phase 2
+
Phase 3
```

也就是：

1. 初始化完整仓库；
2. 建立 Tauri + Rust + React 架构；
3. 建立 CI/lint/test；
4. 实现 Windows battery telemetry；
5. 实现 CLI diagnostics；
6. 实现 battery discharge estimator；
7. 实现 deadline / reserve / power budget / margin 计算；
8. 实现 simulation backend；
9. 编写充分单元测试；
10. 写初版 README。

第一轮结束前：

实际验证能够构建。

最后向我汇报：

```text
Implemented
Tests
Known limitations
Repository structure
How to run diagnostics
How to run simulator
Next phase
```

然后继续推进，而不是只给我一个设计方案。

---

# 57. 最重要的一句话

任何时候发生产品和工程决策冲突，优先遵守：

> **BatteryDeadline should make the user's laptop last until a chosen time with the smallest practical impact on their experience, while never leaving the operating system in a modified state it cannot safely restore.**

现在开始检查仓库并实施 BatteryDeadline。