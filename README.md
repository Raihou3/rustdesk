# RustDesk 局域网增强版

git : [RustDesk 局域网增强原版](https://github.com/thb1314/rustdesk-lan-enhencement)


这个 fork 主要面向可信局域网内更清晰的远程桌面体验。

- 新增 **局域网画质优化**，直连局域网会自动开启，用户可随时关闭恢复原版策略。
- 局域网会使用更高的自定义画质值 `1000`，并在支持时优先使用 **真彩模式（4:4:4）**。
- 新增 **图像锐化**，支持关闭、低、中、高预设，并可用滑杆自定义强度。
- 在会话标题栏和显示质量监测中显示当前连接 IP/路径，方便确认是否为局域网直连。
- Linux Sciter `.deb` 默认带上 `hwcodec` 和 `unix-file-copy-paste`，并补齐 FUSE 运行时依赖。

## 新增：双服务器回退（局域网 + 公网）

当你同时拥有**局域网中继服务器**和**公网中继服务器**时，RustDesk 会自动尝试局域网中继，局域网查不到目标机器时回退到公网中继。

### 配置方式

**方式一：环境变量（适合临时测试）**

```sh
RUSTDESK_LAN_RENDEZVOUS_SERVER=192.168.1.2:21116 \
RUSTDESK_LAN_RELAY_SERVER=192.168.1.2:21117 \
RUSTDESK_LAN_SERVER_TIMEOUT_MS=3000 \
./rustdesk
```

**方式二：配置文件**

```toml
# ~/.local/share/rustdesk/config/RustDesk.toml
lan-rendezvous-server = "192.168.40.70:21116"
lan-relay-server = "192.168.40.70:21117"
lan-server-timeout-ms = 3000
```

**方式三：UI 设置页面**

打开 设置 → ID/Relay Server，填入 LAN Rendezvous Server 和 LAN Relay Server 字段。

### 工作原理

1. 连接时先尝试 TCP 连接局域网中继服务器
2. 如果局域网中继可达，向其查询目标机器
3. 局域网查不到目标 → 自动回退到公网中继重新查询
4. 打洞成功 → P2P 直连；打洞失败 → 走相应的中继服务器转发

### 依赖的构建说明

Ubuntu 26 推荐的打包命令：

```sh
VCPKG_ROOT="./vcpkg" CXXFLAGS="-include cstdint" VULKAN_SDK=/usr RUSTFLAGS="-lswresample" python3 build.py --flutter --hwcodec
```

完整中文说明见 [docs/README-ZH.md](docs/README-ZH.md)。

