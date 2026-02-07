# inventoryAvatars

## 目标

制作一个类似我的世界的物品图标系统预制件，用于在inventory的格子中显示  
保存的数据应该保存到对应的inventory里  

## 具体行为，实现

在鼠标hover的时候显示选中，点击后鼠标离开inventory会自动关闭inventory
并加载对应item的放置hover。

### 固有元数据

- 数量
- 颜色（以后可以被重载为材质贴图）
