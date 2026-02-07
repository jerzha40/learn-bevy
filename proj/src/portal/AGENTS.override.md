# portal

## 目标

制作一个传送门（portal）预制件  
世界泡保存的数据应该包含portal  

## 具体行为，实现

portal是成对出现的，对应传送门两边是相同颜色。  
对应数据结构，一个portal要维护它的另一半，也就是

### 固有数据

- 大小，直径径R/bm
- 检测范围F/bm
- 颜色color
- 指向世界
- 指向的portal（通常是检查指向世界里面有没有这个指向的portal，因为同一个世界可能有很多portal）

例子：
    mainblob有一个portal指向一个新世界newlandblob，和对应portal
    那系统在生成世界阶段就应该在加载mainblob发现这个portal时去检测是否存在newlandblob的存档，
    且那个存档里面是否有对应的portal

玩家鼠标选中传送门出现hover的形态，也就是加一个选择框。  
点击后玩家如果在传送门周围一定范围内（直径2bm）将会被传送，  
也就是先新建窗口，然后把玩家放到新的窗口。
