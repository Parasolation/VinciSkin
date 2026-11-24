# VinciSkin 完整改造补丁包（自动替换任意已拥有皮肤 + 炫彩大图确认弹窗）

## 包含的 4 个改动文件
1. `crates/vinciskin-core/src/static_swap/mod.rs`
   - 新增 `configured_skin()` 与 `overlay_apply_to_slot()`，解除 `skin0` 硬编码，支持替换任意当前选中的已拥有皮肤（含默认皮肤与炫彩）。
2. `src-tauri/src/cn_auto.rs`
   - 选人监听自动读取客户端当前选中的皮肤槽位，并将你在 VinciSkin 中为该英雄设置的目标皮肤写入对应 `skin<编号>.bin`。
3. `src/modules/cn/components/CnSkinGrid.tsx`
   - 点击任意炫彩小图标时弹出大图预览弹窗，完整展示炫彩原图与名称，支持在弹窗内直接切换对比同皮肤下其他炫彩，并提供「取消 / 应用」确认按钮（支持按 Esc 或点击遮罩关闭）。
4. `.github/workflows/build-windows.yml`
   - 免签名 Windows 云端编译工作流，自动移除微软签名配置并编译输出 `.exe` 安装包到 Actions Artifacts。

## 使用步骤
1. Fork `https://github.com/Vinnnnnci/VinciSkin` 到你的 GitHub 账号。
2. 将压缩包中的 4 个文件按相同路径替换/上传到你的 Fork 仓库 `main` 分支。
3. 打开你 Fork 仓库的 **Actions** -> **Build Windows Release (Unsigned)** -> **Run workflow**，编译完成后在页面底部的 **Artifacts** 下载 `VinciSkin-Windows-Release` 即可。
