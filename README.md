<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="VinciSkin" width="96" height="96">
  <h1>VinciSkin</h1>
  <p>英雄联盟国服 mod 管理器</p>
</div>

<div align="center">

[![Sponsor](https://img.shields.io/badge/Sponsor-%E7%88%B1%E5%8F%91%E7%94%B5-946ce6?style=for-the-badge)](https://ifdian.net/a/Vinnnnnci)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue?style=for-the-badge)](LICENSE)
[![Windows 10+](https://img.shields.io/badge/Windows-10+-0078D4?style=for-the-badge&logo=windows)](#)

</div>

VinciSkin 是一个面向英雄联盟国服的 mod 管理器，只支持 Windows 10 和 11。导入 mod，开关、排序、按一下启动，进游戏就能看到效果。自带的 Workshop 可以在同一个窗口里浏览游戏文件、改 mod、打包。

## 国服说明

项目基于国服客户端开发和测试，目前只针对国服。

个人测试跑了一个月，没有出现异常。但不代表没有风险：它通过补丁修改游戏加载的资源，这类做法不符合 Riot 和腾讯的用户协议，存在封号风险。测试结果只覆盖我自己的环境，不保证你的账号安全。用不用、用哪个号，自己判断。

## 功能

- 导入 `.modpkg` 和 `.fantome`，拖进窗口即可
- mod 库：搜索、文件夹、拖拽排序、单个开关
- 配置档：多套 mod 方案随时切换
- mod 健康检查：游戏更新后自动检查，能修的修，修不了的说明原因
- 启动：只启动补丁器，或者一个按钮完成启动
- Workshop：浏览游戏归档、编辑 bin、改文本、打包 mod

## 赞助

软件免费开源，没有付费功能，也不卖授权。如果它帮到了你，可以在爱发电支持：

**[https://ifdian.net/a/Vinnnnnci](https://ifdian.net/a/Vinnnnnci)**

赞助用于代码签名证书和发布开销。如果你是在别处花钱买的，那笔钱没有到我这里，官方版本只在 [Releases](https://github.com/Vinnnnnci/VinciSkin/releases) 发布。

## 参考

- [LeagueToolkit](https://github.com/LeagueToolkit)：mod 格式、哈希表和补丁器的底层库
- [cslol-manager](https://github.com/LeagueToolkit/cslol-manager)：上一代 mod 管理器，注入和启动流程参考了它

## 许可

代码使用 [GPL-3.0-or-later](LICENSE)。名称和图标见 [TRADEMARK.md](TRADEMARK.md)。内置的补丁器二进制遵循 [LTK Patcher License](LTK-PATCHER-LICENSE.md)。

## 下载

安装包发布在 [Releases](https://github.com/Vinnnnnci/VinciSkin/releases) 页面。首个版本还在准备中。

---

问题反馈：[Issues](https://github.com/Vinnnnnci/VinciSkin/issues) · [Discord](https://discord.gg/yhzDVRyQex)
