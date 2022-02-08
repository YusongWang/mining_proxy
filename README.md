<h1 align="center">
  <br>
  <img src="https://raw.githubusercontent.com/YusongWang/mining_proxy/9ec34e9d780866ab8792df09a9d6ec0b0f01b013/images/logo.png" width="350"/>
</h1>

<h2 align="center">全开源 - 无内置开发者钱包</h2>
<h4 align="center">Rust语言编写 基于tokio生态的ETH/ETC/CFX 代理抽水软件</h4>

<p align="center">
  <a>
    <img src="https://img.shields.io/badge/Release-v0.2.2-orgin.svg" alt="travis">
  </a>
  <a>
    <img src="https://img.shields.io/badge/Last_Update-2022_02_08-orgin.svg" alt="travis">
  </a>
  <a>
    <img src="https://img.shields.io/badge/Language-Rust-green.svg" alt="travis">
  </a>
  <a>
    <img src="https://img.shields.io/badge/License-Apache-green.svg" alt="travis">
  </a>
</p>
<p align="center">最新版本见Release <a href="https://github.com/YusongWang/mining_proxy/releases">Github Release</a></p>
<p align="center">历史版本: https://github.com/dothinkdone/mining_proxy/releases</p>
<p align="center">
Coffee: Eth+BSC+HECO+Matic: 0x3602b50d3086edefcd9318bcceb6389004fb14ee
</p>

<p align="center">
作者微信: 13842095202 • Tg: @kk_good
</p>


<p align="center">
  <a href="https://t.me/+ZkUDlH2Fecc3MGM1">Telegram 群</a> •
  <a href="https://jq.qq.com/?_wv=1027&k=AWfknDiw">QQ 群</a> 
</p>

![Screenshot](https://raw.githubusercontent.com/YusongWang/mining_proxy/main/images/web1.jpg)

## :sparkles: 特性

- :cloud: 支持ETH ETC CFX 转发
- :zap: 性能强劲，CPU占用低。
- 💻 可以自定义抽水比例
- 📚 可以自定义抽水算法。
- 💾 安全稳定： 支持TCP SSL 及加密方式(轻量算法，非SSR一类的垃圾东西)3种不通的协议
- :outbox_tray: 一台机器只需要开启一个Web界面。可配置多矿池转发（没有上限）
- :rocket: 開箱即用：All-In-One 打包，一鍵搭建運行，一鍵配置
- :family_woman_girl_boy: 支持Liunx Windows

## :hammer_and_wrench: 部署



Windows 双击运行即可
Liunx 建议用screen 管理或者使用systemctl 进行管理

在软件运行目录下创建 .env 文件
```env
MINING_PROXY_WEB_PORT=8020
MINING_PROXY_WEB_PASSWORD=123456789
JWT_SECRET=test
```
第一行是网页的端口
第二行是网页管理的密码
第三行是登录密码的加密秘钥。建议用随机字符串不少于32位的字符串


## 其他说明
<a href="https://github.com/YusongWang/mining_proxy_web">Web界面地址</a><br>
