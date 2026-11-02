Zhisi / MindWeaver 0.1.0 - Windows x64
=====================================

方式一（推荐，安装到本机）
  1. 解压整个文件夹到任意位置（不要只解压 exe）。
  2. 右键 install.ps1 -> “使用 PowerShell 运行”。
     若提示脚本被禁止，打开 PowerShell 执行：
       powershell -ExecutionPolicy Bypass -File .\install.ps1
  3. 装好后开始菜单 / 桌面会有 MindWeaver 快捷方式，.zmind 文件也会关联到它。
     安装位置：%LOCALAPPDATA%\Programs\MindWeaver（不需要管理员权限）

方式二（免安装）
  直接双击 MindWeaver.exe 即可运行，不写注册表、不建快捷方式。

卸载
  设置 -> 应用 -> “Zhisi (MindWeaver)” -> 卸载
  或执行：powershell -File "%LOCALAPPDATA%\Programs\MindWeaver\install.ps1" -Uninstall

说明
  - 单文件 exe，不依赖额外运行库（不需要 .NET / VC++ 运行时）。
  - 中文字体走系统字体（微软雅黑等），系统缺中文字体时会回退。
  - 文件格式 .zmind 与旧版本兼容：旧文件可直接打开，新文件旧版本也能读（未知键忽略）。
