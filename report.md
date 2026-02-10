# Conway GPU 并行模拟项目报告

> 项目名称：Conway GPU Simulation  
> 作者：Your Name  
> 日期：2026-02-09  
> 仓库地址：<https://github.com/yourname/conway>

---

## 摘要

本项目基于 **Bevy 游戏引擎** 与 **GPU Compute Shader**，实现了一个以网格为基础的并行世界模拟系统。  
项目灵感来源于 *Conway 生命游戏*，并进一步探索其在 **工业 / 物流类模拟（如 Mindustry）** 中的可行性。

与传统 CPU 驱动的逐格更新方式不同，本项目将**世界状态完全存储在 GPU 中**，并通过计算管线并行执行每一帧（tick）的逻辑更新。

---

## 1. 项目背景与动机

### 1.1 研究背景

网格化世界模拟在游戏与仿真领域中非常常见，例如：

- 细胞自动机（Conway 生命游戏）
- 工业/物流模拟（Mindustry、Factorio）
- 地形与扩散系统

随着网格规模的增大，CPU 串行或多线程更新方式逐渐成为性能瓶颈。

---

### 1.2 项目动机

现代 GPU 拥有极强的并行计算能力，但在多数游戏中仅用于图形渲染。  
本项目尝试探索：

> **是否可以将 GPU 作为“世界模拟的主执行单元”，而非仅用于显示？**

---

### 1.3 项目目标

- 构建一个 **GPU 驱动的网格世界模型**
- 使用 **Compute Shader** 完成世界逻辑更新
- 在 **Bevy / wgpu** 框架下完成系统整合
- 为更复杂的工业模拟系统提供基础架构

---

## 2. 技术选型

| 组件       | 说明    |
| ---------- | ------- |
| 编程语言   | Rust    |
| 游戏引擎   | Bevy    |
| 图形后端   | wgpu    |
| 着色器语言 | WGSL    |
| 开发环境   | VS Code |
| 版本控制   | Git     |

---

## 3. 系统总体架构

### 3.1 架构概览

本系统采用 **CPU 调度 + GPU 并行执行** 的总体架构设计。

CPU 侧基于 Bevy 的 ECS 框架，负责系统调度、资源管理与渲染流程控制；  
GPU 侧通过 Compute Shader 执行具体的世界状态更新逻辑。

---

#### 3.1.1 System 设计

在 Bevy 中，**System 并不直接执行世界计算逻辑**，而是作为调度单元存在。

本项目中的 System 主要职责包括：

- 按固定频率（tick）触发世界更新
- 向 Render Graph 提交 Compute Pass
- 负责 Compute Shader 的调度顺序与依赖关系
- 控制世界状态的双缓冲（Ping-Pong）交换

具体的世界计算逻辑完全由 GPU Compute Shader 并行执行，  
CPU 仅作为“调度者”，不参与单个网格单元的计算。

---

#### 3.1.2 Component 设计

在传统 ECS 架构中，Component 通常表示实体的属性数据。  
而在本项目中，Component 的作用发生了转变。

本项目中的 Component 主要用于：

- 持有 GPU 世界状态资源的引用
- 管理 Storage Texture / Buffer 的生命周期
- 为 System 提供对 GPU 数据的访问入口

例如，整个网格世界并非由大量 CPU 实体组成，  
而是以 **单个或少量 GPU Storage Texture** 的形式存在，  
并通过 Component 的方式挂载到 Bevy 世界中。

因此，Component 并不表示“世界中的一个格子”，  
而是表示 **“一整张 GPU 世界状态数据的抽象句柄”**。

### 3.2 字段设计

W1：Building（建筑/占用信息）

bits 0..15 : building_id（0 = none）

bits 16..19 : b_rot（建筑朝向）

bits 20..23 : b_size（1..16，或你自己定义）

bits 24..31 : team_or_owner（0..255，单机先用 0）

解释：Mindustry/Factorio 那类，建筑是动态层核心。你后面还能加“建筑根格/多格占用”规则。

## 4. 功能手册

### 4.1

创建rgbau32 texture

```rust
worldio::create_rgba32u
```

create a resource type of

```rust
ResMut<Assets<Image>>
```
