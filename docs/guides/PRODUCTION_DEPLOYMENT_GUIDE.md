# 生产部署完全指南

## 📋 目录

- [快速开始](#快速开始)
- [本地部署](#本地部署)
- [Docker 自建部署](#docker-自建部署)
- [常见问题排查](#常见问题排查)
- [性能优化](#性能优化)
- [维护和监控](#维护和监控)
- [故障恢复](#故障恢复)

---

## 快速开始

### 系统要求

**桌面版（推荐）**：
- macOS 10.15+ 或 Windows 10+
- 内存：4GB 以上
- 磁盘：1GB 以上

**Web 版**：
- 浏览器：Chrome/Safari/Edge 最新版
- 网络：良好的 HTTP 连接

**服务器部署**：
- Docker 20.10+
- Docker Compose 2.0+
- 内存：2GB 以上
- 磁盘：5GB 以上

### 支持的模型

**推荐模型**（稳定性最好）：
- OpenAI GPT-4 / GPT-4o
- Claude 3 系列
- 国内：DeepSeek V4.1、Kimi K2.5

**支持的模型**（其他生成类）：
- 任何兼容 OpenAI API 的模型
- 任何支持 SSE 流式输出的模型

---

## 本地部署

### 方式 1：使用桌面应用（最简单）

#### 第 1 步：下载应用

前往 [GitHub Releases](https://github.com/baicie/AI-Novel-Writing-Assistant/releases)

选择对应版本：
- **macOS（Apple 芯片）**：文件名含 `mac-arm64`
- **macOS（Intel 芯片）**：文件名含 `mac-x64`
- **Windows**：`.exe` 文件
- **Linux**：`AppImage` 文件

#### 第 2 步：安装应用

**macOS**：
```bash
# 1. 打开 Finder，进入 Downloads 文件夹
# 2. 双击 .dmg 文件
# 3. 将 Biz Novel Studio 拖到 Applications 文件夹
# 4. 打开 Applications，双击应用（首次可能需要允许）

# 或用命令行：
open ~/Downloads/Biz\ Novel\ Studio-*.dmg
```

**Windows**：
```bash
# 1. 双击下载的 .exe 文件
# 2. 按照安装向导完成
# 3. 从开始菜单或桌面启动应用
```

**Linux**：
```bash
# 1. 下载 AppImage 文件
# 2. 设置可执行权限
chmod +x Biz\ Novel\ Studio-*.AppImage

# 3. 运行应用
./Biz\ Novel\ Studio-*.AppImage
```

#### 第 3 步：首次配置

**打开应用后**：

1. **选择创作模式**
   - 点击标题栏右上角的"本地创作"/"云创作"选择器
   - 选择"本地创作"

2. **配置 API 密钥**
   - 进入右上角设置 → "本地创作系统密钥设置"
   - 填入你的 API Key（以下二选一）：
     - **方式 A**：单个模型厂商的 API Key
       ```
       选择厂商（OpenAI / Claude / DeepSeek 等）
       输入 API Key
       选择默认模型
       点击"测试连接"
       ```
     - **方式 B**：使用 OpenCode Go（推荐）
       ```
       选择"OpenCode Go"
       输入一个 OpenCode Go 的 API Key
       可自动访问 5+ 个模型提供商
       ```

3. **开始写作**
   - 创建或打开小说
   - 点击右上角"本地创作"确认当前模式
   - 正常使用所有功能

### 方式 2：从源代码本地运行

#### 系统要求

```bash
# macOS / Linux
- Node.js 20+
- pnpm 10.6+
- Rust 1.70+
- Python 3.9+（可选）

# Windows
- 同上，另需 Visual Studio Build Tools
```

#### 安装步骤

```bash
# 1. 克隆仓库
git clone https://github.com/baicie/AI-Novel-Writing-Assistant.git
cd AI-Novel-Writing-Assistant

# 2. 安装依赖
pnpm install

# 3. 启动开发服务器
pnpm dev

# 应用会在 3000 端口启动
# 打开浏览器访问 http://localhost:3000
```

#### 首次启动

如果遇到错误，请检查：

```bash
# 检查依赖
pnpm --filter "*" check

# 重新安装
pnpm clean
pnpm install

# 查看日志
pnpm dev:log

# 完整构建
pnpm build
```

---

## Docker 自建部署

### 方式 1：使用 Docker Compose（推荐）

#### 第 1 步：准备文件

```bash
# 1. 创建部署目录
mkdir ai-novel-server
cd ai-novel-server

# 2. 下载 docker-compose.yml
curl -o docker-compose.yml https://raw.githubusercontent.com/baicie/AI-Novel-Writing-Assistant/main/docker-compose.yml

# 3. 复制环境文件
curl -o .env.example https://raw.githubusercontent.com/baicie/AI-Novel-Writing-Assistant/main/server/.env.example
cp .env.example .env
```

#### 第 2 步：配置环境

编辑 `.env` 文件：

```bash
# 应用配置
NODE_ENV=production
PORT=3000
HOST=0.0.0.0

# 数据库（Docker 会自动创建）
DB_PATH=/data/novels.db

# API 密钥（可选，如果设置所有 API 请求都需要此密钥）
API_KEY=your-secure-key-here

# LLM 厂商配置（可选，用于云模式）
OPENAI_API_KEY=your-openai-key
CLAUDE_API_KEY=your-claude-key
DEEPSEEK_API_KEY=your-deepseek-key

# 访问密码（可选，用于保护 Web 端）
WEB_PASSWORD=your-secure-password

# 知识库（可选）
QDRANT_URL=http://qdrant:6333
QDRANT_COLLECTION=novels
```

#### 第 3 步：启动服务

```bash
# 启动所有服务
docker-compose up -d

# 查看日志
docker-compose logs -f

# 停止服务
docker-compose down

# 访问应用
# Web: http://localhost:3000
# API: http://localhost:3000/api
```

#### 检查状态

```bash
# 查看运行中的容器
docker-compose ps

# 查看应用日志
docker-compose logs app

# 进入容器调试
docker-compose exec app sh
```

### 方式 2：使用 Docker 单个容器

如果只想运行应用容器：

```bash
# 构建镜像
docker build -f Dockerfile.api -t ai-novel:latest .

# 运行容器
docker run -d \
  --name ai-novel \
  -p 3000:3000 \
  -v ai-novel-data:/data \
  -e NODE_ENV=production \
  -e PORT=3000 \
  ai-novel:latest

# 查看日志
docker logs -f ai-novel

# 停止容器
docker stop ai-novel
docker rm ai-novel
```

### 方式 3：使用 Kubernetes

如果你使用 Kubernetes 部署：

```bash
# 1. 创建命名空间
kubectl create namespace ai-novel

# 2. 创建配置和密钥
kubectl create configmap ai-novel-config \
  --from-env-file=.env \
  -n ai-novel

# 3. 部署应用（使用 Helm 或 YAML）
# 详见 k8s/ 目录中的部署文件

kubectl apply -f k8s/deployment.yaml -n ai-novel
kubectl apply -f k8s/service.yaml -n ai-novel
```

---

## 常见问题排查

### 应用启动问题

#### Q1：无法连接到 API Key

**症状**：
- 显示"连接失败"或"API 密钥无效"
- 测试连接超时

**排查步骤**：

```bash
# 1. 检查网络连接
ping api.openai.com  # 或其他厂商

# 2. 检查 API Key 是否正确
# 方法：复制粘贴到记事本，检查是否有多余空格

# 3. 检查 API Key 是否有效期
# 登录对应厂商的控制台确认

# 4. 检查是否在 API Key 受限 IP 白名单中
# 如果设置了 IP 限制，需要添加你的 IP

# 5. 测试 API 连接（以 OpenAI 为例）
curl -H "Authorization: Bearer YOUR_API_KEY" \
  https://api.openai.com/v1/models
```

**解决方案**：
1. 确认 API Key 无多余空格
2. 检查 Key 的有效期和额度
3. 检查 IP 白名单设置
4. 尝试在浏览器测试 API 端点
5. 检查是否需要代理（代理配置在高级设置中）

#### Q2：应用闪退或无法启动

**症状**：
- 应用启动后立即关闭
- 显示"应用程序出错"

**排查步骤**：

```bash
# macOS：查看系统日志
log stream --predicate 'eventMessage contains "ai-novel"'

# Windows：查看事件查看器
# 打开事件查看器 → Windows 日志 → 系统

# Linux：查看系统日志
journalctl -u ai-novel -f

# 或查看应用日志目录
# macOS: ~/Library/Logs/Biz\ Novel\ Studio/
# Windows: %APPDATA%\Biz Novel Studio\logs\
# Linux: ~/.local/share/Biz\ Novel\ Studio/logs/
```

**常见原因和解决**：
- **原因**：端口被占用
  ```bash
  # 检查 3000 端口
  lsof -i :3000  # macOS/Linux
  netstat -ano | findstr :3000  # Windows
  
  # 解决：改用其他端口或关闭占用的进程
  ```

- **原因**：数据库损坏
  ```bash
  # 解决：删除数据库文件，重新初始化
  rm ~/Library/Application\ Support/Biz\ Novel\ Studio/novels.db
  # 重启应用
  ```

- **原因**：权限不足
  ```bash
  # macOS：检查权限
  ls -la /Applications/Biz\ Novel\ Studio.app/
  
  # 重新安装应用
  ```

#### Q3：Web 版无法访问

**症状**：
- 浏览器显示"无法访问此网站"
- 连接超时

**排查步骤**：

```bash
# 1. 检查服务是否运行
docker-compose ps
docker ps | grep ai-novel

# 2. 检查端口是否开放
netstat -an | grep 3000
ss -tlnp | grep 3000

# 3. 检查防火墙规则
# macOS
sudo pfctl -sr | grep 3000

# Linux
sudo ufw status
sudo iptables -L | grep 3000

# Windows
netsh advfirewall firewall show all | findstr 3000

# 4. 检查应用日志
docker-compose logs app
tail -100f /var/log/ai-novel/app.log

# 5. 测试本地连接
curl http://localhost:3000
curl http://127.0.0.1:3000

# 6. 测试远程连接
curl http://<server-ip>:3000
```

**解决方案**：
1. 启动服务：`docker-compose up -d`
2. 开放防火墙：允许 3000 端口入站
3. 检查网络：确保浏览器和服务器在同一网络
4. 尝试 IP 而不是域名：`http://<ip>:3000`

### 生成和功能问题

#### Q4：本地生成超时

**症状**：
- 生成中途卡住
- 显示"生成超时"

**排查步骤**：

```bash
# 1. 检查网络延迟
ping api.openai.com
# 如果延迟 > 2000ms，可能有问题

# 2. 查看 API 使用情况
# 登录对应厂商控制台，检查：
# - 当前使用额度
# - 速率限制（RPM/TPM）
# - 账户是否在黑名单

# 3. 调整超时时间
# 设置 → 高级选项 → 生成超时
# 改为更大的值（如 120000ms）

# 4. 检查本地资源
# 检查 CPU 和内存使用
top  # macOS/Linux
tasklist  # Windows
```

**解决方案**：
1. 增加超时时间
2. 检查 API 账户是否正常
3. 减小模型的 max_tokens
4. 尝试更快的网络
5. 在非繁忙时段重试

#### Q5：候选方案为空或错误

**症状**：
- 生成后没有候选方案
- 显示 JSON 解析错误

**排查步骤**：

```bash
# 1. 查看生成日志
# 设置 → 开发者工具 → 查看日志

# 2. 检查 API 响应
# 打开浏览器开发者工具
# F12 → Network → 查看最后一个 /generate 请求的响应

# 3. 验证模型支持
# 某些模型可能不支持结构化输出
# 尝试切换到推荐模型

# 4. 检查上下文大小
# 如果上下文太大，可能导致 JSON 格式错误
```

**解决方案**：
1. 使用推荐的模型（GPT-4, Claude 3+）
2. 减小书籍上下文或章节长度
3. 在设置中启用 JSON 修复选项
4. 查看详细错误日志

### 性能问题

#### Q6：应用响应缓慢

**症状**：
- UI 卡顿
- 列表加载慢

**排查步骤**：

```bash
# 1. 检查系统资源
# macOS
top -u ai-novel

# Windows
tasklist | findstr "Biz"
# 或在任务管理器中查看

# 2. 检查磁盘空间
df -h  # Unix
diskutil list  # macOS
wmic logicaldisk get name,freespace  # Windows

# 3. 检查数据库大小
ls -lh ~/Library/Application\ Support/Biz\ Novel\ Studio/novels.db

# 4. 清理缓存
# 设置 → 存储 → 清空缓存
```

**解决方案**：
1. 关闭其他应用释放内存
2. 清空临时文件和缓存
3. 优化数据库（见下面的维护章节）
4. 检查是否需要升级硬件

---

## 性能优化

### 性能基准

在标准配置下的性能基准：

```
应用启动时间：< 5 秒
首页加载时间：< 2 秒
书籍列表加载（100+ 本书）：< 3 秒
章节内容加载（50k 字）：< 1 秒
生成响应时间（第一个 token）：2-5 秒
```

### 优化建议

#### 1. 本地优化

```bash
# 减少 UI 重绘
# 设置 → 性能 → 启用虚拟滚动

# 清理数据库
# 设置 → 存储 → 优化数据库

# 禁用不必要的功能
# 设置 → 功能 → 关闭知识库索引（如不使用）

# 使用本地存储而不是网络存储
# 避免将数据保存在网络驱动器上
```

#### 2. API 优化

```bash
# 使用缓存提示词（Claude）
# 在高级选项中启用 cache_control

# 使用流式输出（所有模型）
# 默认启用，无需配置

# 批量处理请求
# 避免频繁的小请求，改为一次性请求多个

# 使用更快的模型
# GPT-4o < GPT-4
# Claude 3.5 Sonnet 比 Claude 3 Opus 快
```

#### 3. 服务器优化

```bash
# 数据库优化
# 定期运行维护任务（见下面的维护章节）

# 内存优化
# 根据并发用户数调整 Node.js 堆内存
NODE_OPTIONS="--max-old-space-size=2048"

# 缓存优化
# 启用 Redis 缓存（如部署）
REDIS_URL=redis://localhost:6379

# CDN 优化
# 静态资源通过 CDN 分发
STATIC_CDN_URL=https://cdn.example.com
```

### 性能监控

#### 监控关键指标

```bash
# 应用性能
# 打开开发者工具 → Performance
# 记录性能快照

# 内存使用
# 开发者工具 → Memory
# 拍摄堆快照，检查内存泄漏

# 网络性能
# 开发者工具 → Network
# 监控 API 响应时间

# 数据库性能
# 查看查询日志
tail -f /var/log/ai-novel/database.log

# 服务器性能
# CPU 使用率
top

# 磁盘 I/O
iostat -x 1

# 网络连接
netstat -s
```

---

## 维护和监控

### 日常维护

#### 数据库维护

```bash
# 1. 定期备份
# 备份数据库
cp ~/Library/Application\ Support/Biz\ Novel\ Studio/novels.db \
   ~/Backups/novels-$(date +%Y%m%d).db.bak

# 2. 优化数据库
# 在 Web 端：设置 → 存储 → 优化数据库
# 或通过 API：POST /api/maintenance/optimize-db

# 3. 检查数据完整性
# 在 Web 端：设置 → 存储 → 检查数据完整性
# 或通过 API：POST /api/maintenance/check-integrity

# 4. 清理过期数据
# 在 Web 端：设置 → 存储 → 清理垃圾
# 自动清理 30 天前的临时文件
```

#### 日志管理

```bash
# 日志位置
# macOS: ~/Library/Logs/Biz\ Novel\ Studio/
# Windows: %APPDATA%\Biz Novel Studio\logs\
# Linux: ~/.local/share/Biz\ Novel\ Studio/logs/

# 日志轮转（自动）
# 日志每天自动轮转
# 保留最近 30 天的日志

# 手动清理日志
# 清理超过 30 天的日志
find ~/.local/share/Biz\ Novel\ Studio/logs -mtime +30 -delete

# 导出日志用于分析
# 设置 → 开发者工具 → 导出日志
```

#### 安全维护

```bash
# 定期更新应用
# 启用自动更新检查
# 设置 → 更新 → 启用自动检查

# 更新 API Key（定期）
# 建议每 90 天更新一次 API Key

# 备份密钥配置
# 如果启用了系统凭据存储
# 定期备份钥匙链/凭据管理器

# 审计日志
# 检查错误日志中的异常
# 设置 → 开发者工具 → 查看错误日志
```

### 监控指标

#### 应用监控

```bash
# 应用健康检查
# API: GET /api/health
# 响应示例：
# {
#   "status": "healthy",
#   "version": "0.5.7",
#   "uptime": 86400,
#   "memory": "245 MB",
#   "database": "healthy"
# }

# 使用指标
# API: GET /api/metrics
# - 总书籍数
# - 总章节数
# - 生成次数
# - 平均生成时间

# 错误率监控
# API: GET /api/metrics/errors
# - 最近 24 小时的错误数
# - 错误类型分布
# - 影响最大的错误
```

#### 告警配置

```bash
# 高 CPU 使用率告警
# 如果 CPU > 80% 持续 5 分钟，发送告警

# 高内存使用率告警
# 如果内存 > 90%，发送告警

# 磁盘空间不足告警
# 如果可用空间 < 1GB，发送告警

# API 响应时间告警
# 如果平均响应时间 > 2 秒，发送告警

# 错误率告警
# 如果错误率 > 1%，发送告警

# 配置告警
# 通过 webhook 发送告警
ALERT_WEBHOOK=https://example.com/alerts
```

---

## 故障恢复

### 备份和恢复

#### 完整备份

```bash
# 备份所有数据
tar -czf ai-novel-backup-$(date +%Y%m%d-%H%M%S).tar.gz \
  ~/Library/Application\ Support/Biz\ Novel\ Studio/ \
  ~/.local/share/Biz\ Novel\ Studio/

# 验证备份
tar -tzf ai-novel-backup-*.tar.gz | head -20

# 恢复备份
tar -xzf ai-novel-backup-*.tar.gz -C ~/
```

#### 数据库恢复

```bash
# 从备份恢复数据库
cp ~/Backups/novels-20240101.db.bak \
   ~/Library/Application\ Support/Biz\ Novel\ Studio/novels.db

# 验证恢复
# 重启应用并检查数据是否完整

# 如果恢复失败，重新初始化
rm ~/Library/Application\ Support/Biz\ Novel\ Studio/novels.db
# 重启应用，重新初始化数据库
```

### 常见故障和恢复

#### 故障 1：数据库损坏

**症状**：
- 应用启动时显示"数据库错误"
- 无法加载任何书籍

**恢复步骤**：

```bash
# 1. 尝试修复数据库
# 设置 → 存储 → 修复数据库

# 2. 如果仍然失败，从备份恢复
cp ~/Backups/novels-latest.db.bak \
   ~/Library/Application\ Support/Biz\ Novel\ Studio/novels.db

# 3. 如果没有备份，重新初始化
rm ~/Library/Application\ Support/Biz\ Novel\ Studio/novels.db
# 重启应用

# 4. 联系支持团队获取帮助
```

#### 故障 2：API 密钥泄露

**症状**：
- 发现 API Key 被公开
- API 报告异常使用

**恢复步骤**：

```bash
# 1. 立即撤销密钥
# 登录 API 厂商控制台，删除泄露的 Key

# 2. 生成新密钥
# 在厂商控制台创建新的 API Key

# 3. 更新应用配置
# 设置 → 本地创作系统密钥设置 → 更新 API Key

# 4. 清除旧凭据
# 清除系统凭据存储中的旧 Key
# macOS: Keychain Access
# Windows: 凭据管理器
# Linux: pass / gnome-keyring

# 5. 检查使用日志
# 查看 API 账户的使用记录，检查异常
```

#### 故障 3：服务完全崩溃

**症状**：
- 应用无法启动
- 所有功能都不可用

**恢复步骤**：

```bash
# 1. 重启应用
# 通常能解决大部分问题

# 2. 检查系统日志
log stream --predicate 'eventMessage contains "ai-novel"'

# 3. 尝试重新安装
# 卸载应用
# 清除应用数据（如果有备份）
rm -rf ~/Library/Application\ Support/Biz\ Novel\ Studio/

# 重新安装应用
# 从 GitHub Releases 下载最新版本

# 4. 从备份恢复数据
# 安装后，将备份的 novels.db 恢复
cp ~/Backups/novels-latest.db.bak \
   ~/Library/Application\ Support/Biz\ Novel\ Studio/novels.db

# 5. 联系支持
# 如果仍未解决，请报告问题
```

### 获取技术支持

#### 收集诊断信息

```bash
# 系统信息
uname -a
sw_vers  # macOS
# 或用 settings → about

# 应用版本
# 设置 → 关于 → 查看版本号

# 错误日志
# 导出日志文件
# 设置 → 开发者工具 → 导出日志

# 应用日志完整导出
tar -czf diagnostics-$(date +%Y%m%d).tar.gz \
  ~/Library/Logs/Biz\ Novel\ Studio/ \
  ~/Library/Application\ Support/Biz\ Novel\ Studio/logs/

# API 请求日志
# 打开浏览器开发者工具 F12
# Network 标签页，保存 HAR 文件
```

#### 报告问题

**通过 GitHub Issues**：
1. 进入 [GitHub Issues](https://github.com/baicie/AI-Novel-Writing-Assistant/issues)
2. 点击"New Issue"
3. 选择合适的 Issue 模板
4. 填写详细信息：
   - 问题描述
   - 重现步骤
   - 系统信息
   - 错误日志
   - 截图或视频

**联系支持团队**：
- 邮件：support@example.com
- 论坛：https://community.example.com
- Discord：[邀请链接]

---

## 附录

### A. 环境变量完整列表

```bash
# 应用配置
NODE_ENV=production          # 运行环境
PORT=3000                    # 监听端口
HOST=0.0.0.0                 # 监听地址

# 数据库
DB_PATH=/data/novels.db      # 数据库路径
DB_POOL_SIZE=10              # 连接池大小
DB_TIMEOUT=30000             # 查询超时

# API 配置
API_BASE_URL=http://localhost:3000
API_TIMEOUT=30000            # API 超时
API_KEY=                      # 可选：API 认证密钥

# LLM 厂商（用于云模式）
OPENAI_API_KEY=              # OpenAI API Key
CLAUDE_API_KEY=              # Claude API Key
DEEPSEEK_API_KEY=            # DeepSeek API Key
KIMI_API_KEY=                # Kimi API Key

# 服务配置
LOG_LEVEL=info               # 日志级别
LOG_PATH=/logs               # 日志目录
CACHE_TTL=3600               # 缓存有效期

# 安全配置
WEB_PASSWORD=                # 可选：Web 访问密码
HTTPS_ENABLED=false          # 启用 HTTPS
SSL_CERT_PATH=               # SSL 证书路径
CORS_ORIGIN=*                # CORS 跨域配置

# 知识库（可选）
QDRANT_URL=http://qdrant:6333
QDRANT_API_KEY=
QDRANT_COLLECTION=novels

# Redis 缓存（可选）
REDIS_URL=redis://localhost:6379
REDIS_PASSWORD=

# 监控和日志
SENTRY_DSN=                  # Sentry 错误追踪
DATADOG_API_KEY=             # Datadog 监控
```

### B. Docker 常用命令

```bash
# 查看日志
docker-compose logs -f app
docker-compose logs --tail=100 app

# 进入容器
docker-compose exec app sh
docker-compose exec app bash

# 重启服务
docker-compose restart app

# 停止特定服务
docker-compose stop app

# 删除容器（保留数据卷）
docker-compose down

# 删除一切包括数据卷
docker-compose down -v

# 查看资源使用
docker stats

# 进行备份
docker-compose exec app tar czf - /data | gzip > backup.tar.gz

# 恢复备份
docker-compose exec app tar xzf - < backup.tar.gz
```

### C. 性能调优参数

```bash
# Node.js 堆内存
NODE_OPTIONS="--max-old-space-size=2048 --max-http-header-size=8192"

# 数据库连接池
DB_POOL_SIZE=20              # 增加并发连接数

# API 超时
API_TIMEOUT=60000            # 增加用于大型请求

# 日志级别（生产环境）
LOG_LEVEL=warn               # 减少日志 I/O

# 缓存策略
CACHE_TTL=7200               # 增加缓存有效期
CACHE_SIZE=1000              # 内存缓存大小

# 数据库优化
DB_QUERY_TIMEOUT=15000       # 单个查询超时
DB_TRANSACTION_TIMEOUT=30000 # 事务超时
```

### D. 安全最佳实践

```bash
# 1. 使用强密码
WEB_PASSWORD=$(openssl rand -base64 32)

# 2. 启用 HTTPS
HTTPS_ENABLED=true
SSL_CERT_PATH=/etc/ssl/certs/server.crt
SSL_KEY_PATH=/etc/ssl/private/server.key

# 3. 配置防火墙
# 只允许特定 IP 访问
ufw allow from 203.0.113.0 to any port 3000

# 4. 定期备份
0 2 * * * /scripts/backup.sh

# 5. 监控错误日志
0 * * * * /scripts/check-errors.sh

# 6. 定期更新依赖
# 运行：npm audit fix

# 7. 使用密钥管理服务
# 不要在环境变量中硬编码敏感信息
# 使用 HashiCorp Vault 或类似服务
```

---

## 支持和反馈

- **文档**：https://docs.example.com
- **社区论坛**：https://community.example.com
- **GitHub Issues**：https://github.com/baicie/AI-Novel-Writing-Assistant/issues
- **邮件支持**：support@example.com

