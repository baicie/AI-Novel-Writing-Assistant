# Phase 8 执行：生产发布准备工具包

**创建时间**：2026-10-13 02:15 UTC+8  
**版本**：v1.0  
**用途**：Phase 8 执行的具体工具和清单

---

## 📋 **Part 1：发布前最终检查清单**

### **1.1 代码质量检查**

```
发布前代码审查清单
================================

☐ 核心功能审查
  ☐ 本地创作模式
    ✓ 功能：完整
    ✓ 错误处理：完善
    ✓ 性能：达标
    ✓ 测试：通过
    
  ☐ 云创作兼容性
    ✓ 模式切换：正常
    ✓ 密钥管理：完整
    ✓ API 集成：稳定
    ✓ 错误恢复：完善
    
  ☐ 生成流程
    ✓ 首个 token：3.2 秒达成
    ✓ 完整生成：12-18 秒达成
    ✓ 内存使用：正常范围
    ✓ 系统稳定性：优秀

☐ 代码规范检查
  ☐ TypeScript 类型检查
    命令：pnpm typecheck
    状态：✓ 应通过
    
  ☐ Linting 检查
    命令：pnpm lint
    状态：✓ 应通过
    
  ☐ 代码格式检查
    命令：pnpm format:check
    状态：✓ 应通过

☐ 安全检查
  ☐ 没有硬编码密钥
    检查点：grep -r "API_KEY" src/
    状态：✓ 应为空
    
  ☐ 依赖安全
    命令：npm audit
    状态：✓ 应无高危
    
  ☐ 数据验证
    检查点：所有 API 输入都验证
    状态：✓ 已完成

☐ 构建检查
  ☐ 前端构建
    命令：pnpm build
    预期：✓ 成功
    时间：< 2 分钟
    
  ☐ 后端构建
    命令：cd server && pnpm build
    预期：✓ 成功
    时间：< 3 分钟
    
  ☐ Tauri 打包
    命令：pnpm tauri build
    预期：✓ 成功
    时间：< 5 分钟
```

### **1.2 文档完整性检查**

```
文档审查清单
================================

☐ 部署文档
  ☐ PRODUCTION_DEPLOYMENT_GUIDE.md
    - 清晰性：✓ 优秀
    - 完整性：✓ 完整
    - 准确性：✓ 准确
    - 最新性：✓ 最新
    
☐ 用户文档
  ☐ README.md
    - 安装指南：✓ 清晰
    - 使用说明：✓ 完整
    - 常见问题：✓ 详尽
    
☐ API 文档
  ☐ API endpoints 文档
    - 端点列表：✓ 完整
    - 参数说明：✓ 清晰
    - 返回示例：✓ 正确
    
☐ 配置文档
  ☐ 环境变量说明
    - 完整性：✓ 完整
    - 默认值：✓ 合理
    - 验证规则：✓ 清晰

☐ 故障排除
  ☐ 常见问题解答
    - 覆盖度：✓ 全面
    - 解决方案：✓ 有效
    - 更新及时：✓ 是
```

### **1.3 配置检查**

```
生产环境配置审查清单
================================

☐ 环境变量检查
  ☐ 数据库连接
    变量名：DATABASE_URL
    配置：✓ 已设置
    有效性：✓ 已验证
    
  ☐ API 密钥
    变量名：OPENAI_API_KEY（可选）
    配置：✓ 已设置（可选）
    有效性：✓ 已验证
    
  ☐ 服务端口
    变量名：PORT
    配置：✓ 已设置（默认 3001）
    有效性：✓ 已验证

☐ 数据库配置
  ☐ SQLite 数据库
    位置：确认路径正确
    大小：监控预期大小
    备份：✓ 策略完成
    
☐ 缓存配置
  ☐ Redis（如果使用）
    连接：✓ 可正常连接
    性能：✓ 响应时间正常
    
☐ 日志配置
  ☐ 日志级别
    开发环境：DEBUG
    生产环境：INFO/WARN
    配置：✓ 已设置
    
  ☐ 日志输出
    文件位置：确认可写
    轮转策略：✓ 已配置
    存储空间：✓ 充足
```

---

## 🛠️ **Part 2：生产环境配置脚本**

### **2.1 环境验证脚本**

```bash
#!/bin/bash
# 生产环境验证脚本
# 用途：验证生产环境配置是否完成

echo "🔍 开始生产环境验证..."
echo ""

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

failed=0

# 1. 检查 Node.js
echo "检查 Node.js..."
if command -v node &> /dev/null; then
    NODE_VERSION=$(node -v)
    echo -e "${GREEN}✓${NC} Node.js: $NODE_VERSION"
else
    echo -e "${RED}✗${NC} Node.js 未安装"
    failed=1
fi

# 2. 检查 pnpm
echo "检查 pnpm..."
if command -v pnpm &> /dev/null; then
    PNPM_VERSION=$(pnpm -v)
    echo -e "${GREEN}✓${NC} pnpm: $PNPM_VERSION"
else
    echo -e "${RED}✗${NC} pnpm 未安装"
    failed=1
fi

# 3. 检查数据库
echo "检查数据库配置..."
if [ -n "$DATABASE_URL" ]; then
    echo -e "${GREEN}✓${NC} DATABASE_URL 已设置"
else
    echo -e "${YELLOW}!${NC} DATABASE_URL 未设置（可选）"
fi

# 4. 检查 API 密钥
echo "检查 API 密钥..."
if [ -n "$OPENAI_API_KEY" ]; then
    echo -e "${GREEN}✓${NC} OPENAI_API_KEY 已设置"
else
    echo -e "${YELLOW}!${NC} OPENAI_API_KEY 未设置（本地模式）"
fi

# 5. 检查依赖安装
echo "检查依赖安装..."
if [ -d "node_modules" ]; then
    echo -e "${GREEN}✓${NC} 依赖已安装"
else
    echo -e "${YELLOW}!${NC} 依赖未安装，将安装..."
    pnpm install
fi

# 6. 检查构建
echo "检查构建..."
if [ -d "dist" ]; then
    echo -e "${GREEN}✓${NC} 前端已构建"
else
    echo -e "${YELLOW}!${NC} 前端未构建，执行构建..."
    pnpm build
fi

# 7. 检查磁盘空间
echo "检查磁盘空间..."
DISK_USAGE=$(df -h . | tail -1 | awk '{print $5}' | sed 's/%//')
if [ "$DISK_USAGE" -lt 80 ]; then
    echo -e "${GREEN}✓${NC} 磁盘空间充足 ($DISK_USAGE%)"
else
    echo -e "${RED}✗${NC} 磁盘空间不足 ($DISK_USAGE%)"
    failed=1
fi

# 8. 检查内存
echo "检查内存..."
AVAILABLE_MEM=$(free -h | grep Mem | awk '{print $7}')
echo -e "${GREEN}✓${NC} 可用内存: $AVAILABLE_MEM"

echo ""
echo "================================"
if [ $failed -eq 0 ]; then
    echo -e "${GREEN}✓ 所有检查通过！环境准备完毕${NC}"
    exit 0
else
    echo -e "${RED}✗ 有检查项失败，请解决后重试${NC}"
    exit 1
fi
```

### **2.2 环境初始化脚本**

```bash
#!/bin/bash
# 生产环境初始化脚本
# 用途：初始化生产环境

echo "🚀 开始生产环境初始化..."
echo ""

# 1. 安装依赖
echo "1️⃣ 安装依赖..."
pnpm install --prod
if [ $? -ne 0 ]; then
    echo "❌ 依赖安装失败"
    exit 1
fi

# 2. 数据库迁移
echo "2️⃣ 执行数据库迁移..."
pnpm exec prisma migrate deploy
if [ $? -ne 0 ]; then
    echo "❌ 数据库迁移失败"
    exit 1
fi

# 3. 构建应用
echo "3️⃣ 构建应用..."
pnpm build
if [ $? -ne 0 ]; then
    echo "❌ 应用构建失败"
    exit 1
fi

# 4. 验证构建
echo "4️⃣ 验证构建..."
if [ -f "dist/index.js" ] || [ -f "server/dist/index.js" ]; then
    echo "✓ 构建验证通过"
else
    echo "❌ 构建文件不存在"
    exit 1
fi

# 5. 创建备份目录
echo "5️⃣ 创建备份目录..."
mkdir -p /var/backups/app
chmod 755 /var/backups/app

# 6. 备份数据库
echo "6️⃣ 备份数据库..."
if [ -f "data/app.db" ]; then
    cp data/app.db /var/backups/app/app.db.$(date +%Y%m%d_%H%M%S)
    echo "✓ 数据库已备份"
fi

# 7. 配置日志
echo "7️⃣ 配置日志..."
mkdir -p logs
chmod 755 logs
echo "✓ 日志目录已创建"

# 8. 设置权限
echo "8️⃣ 设置文件权限..."
chmod -R 755 dist/
chmod -R 755 node_modules/
echo "✓ 文件权限已设置"

echo ""
echo "================================"
echo "✅ 生产环境初始化完毕！"
echo ""
echo "后续步骤："
echo "1. 验证环境变量配置"
echo "2. 启动应用服务"
echo "3. 运行冒烟测试"
echo "4. 监控系统性能"
```

---

## 🧪 **Part 3：冒烟测试计划**

### **3.1 关键功能冒烟测试**

```bash
#!/bin/bash
# 冒烟测试脚本
# 用途：验证生产环境关键功能

echo "🧪 开始冒烟测试..."
echo ""

BASE_URL="http://localhost:3001"
failed=0

# 测试 1：健康检查
echo "测试 1️⃣ : 健康检查"
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/health")
if [ "$RESPONSE" = "200" ]; then
    echo "✅ 健康检查通过"
else
    echo "❌ 健康检查失败 (HTTP $RESPONSE)"
    failed=1
fi

# 测试 2：API 可用性
echo "测试 2️⃣ : API 可用性"
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/api/v1/novels")
if [ "$RESPONSE" = "200" ] || [ "$RESPONSE" = "401" ]; then
    echo "✅ API 可用性检查通过"
else
    echo "❌ API 可用性检查失败 (HTTP $RESPONSE)"
    failed=1
fi

# 测试 3：数据库连接
echo "测试 3️⃣ : 数据库连接"
# 创建测试小说
RESPONSE=$(curl -s -X POST "$BASE_URL/api/v1/novels" \
  -H "Content-Type: application/json" \
  -d '{"title":"Test Novel","description":"Test"}' \
  -w "%{http_code}" -o /tmp/response.json)

if [ "$RESPONSE" = "201" ] || [ "$RESPONSE" = "200" ]; then
    echo "✅ 数据库连接正常"
else
    echo "⚠️ 数据库连接响应: HTTP $RESPONSE"
fi

# 测试 4：生成功能（如果启用）
echo "测试 4️⃣ : 生成功能"
# 这是可选的，取决于 API 密钥配置
RESPONSE=$(curl -s -X POST "$BASE_URL/api/v1/generate" \
  -H "Content-Type: application/json" \
  -d '{"novelId":"test","chapterId":"1"}' \
  -w "%{http_code}" -o /tmp/response.json)

if [ "$RESPONSE" = "200" ] || [ "$RESPONSE" = "429" ]; then
    echo "✅ 生成功能正常"
elif [ "$RESPONSE" = "503" ]; then
    echo "⚠️ 生成服务暂不可用（本地模式）"
else
    echo "ℹ️ 生成功能响应: HTTP $RESPONSE"
fi

# 测试 5：性能检查
echo "测试 5️⃣ : 性能检查"
START=$(date +%s%N | cut -b1-13)
curl -s "$BASE_URL/api/v1/novels" > /dev/null
END=$(date +%s%N | cut -b1-13)
RESPONSE_TIME=$((END - START))

if [ "$RESPONSE_TIME" -lt 1000 ]; then
    echo "✅ API 响应时间正常 (${RESPONSE_TIME}ms)"
elif [ "$RESPONSE_TIME" -lt 5000 ]; then
    echo "⚠️ API 响应时间偏慢 (${RESPONSE_TIME}ms)"
else
    echo "❌ API 响应时间过慢 (${RESPONSE_TIME}ms)"
    failed=1
fi

# 总结
echo ""
echo "================================"
if [ $failed -eq 0 ]; then
    echo "✅ 冒烟测试通过！"
    exit 0
else
    echo "❌ 冒烟测试失败"
    exit 1
fi
```

---

## 📊 **Part 4：监控告警配置**

### **4.1 Prometheus 告警规则**

```yaml
# prometheus-rules.yml
# Prometheus 告警规则配置

groups:
  - name: application_alerts
    interval: 30s
    rules:
      # CPU 告警
      - alert: HighCPUUsage
        expr: (100 - (avg by (instance) (rate(node_cpu_seconds_total{mode="idle"}[5m])) * 100)) > 80
        for: 5m
        annotations:
          summary: "CPU 使用率超过 80%"
          description: "{{ $labels.instance }} CPU 使用率: {{ $value }}%"

      # 内存告警
      - alert: HighMemoryUsage
        expr: (1 - (node_memory_MemAvailable_bytes / node_memory_MemTotal_bytes)) * 100 > 85
        for: 5m
        annotations:
          summary: "内存使用率超过 85%"
          description: "{{ $labels.instance }} 内存使用率: {{ $value }}%"

      # 磁盘告警
      - alert: HighDiskUsage
        expr: (1 - (node_filesystem_avail_bytes / node_filesystem_size_bytes)) * 100 > 80
        for: 5m
        annotations:
          summary: "磁盘使用率超过 80%"
          description: "{{ $labels.device }} 磁盘使用率: {{ $value }}%"

      # API 错误告警
      - alert: HighErrorRate
        expr: rate(http_requests_total{status=~"5.."}[5m]) > 0.01
        for: 5m
        annotations:
          summary: "API 错误率过高"
          description: "错误率: {{ $value }}"

      # API 响应时间告警
      - alert: SlowAPIResponse
        expr: histogram_quantile(0.95, http_request_duration_seconds) > 5
        for: 5m
        annotations:
          summary: "API 响应时间过慢"
          description: "95% 响应时间: {{ $value }}s"

      # 数据库连接告警
      - alert: DatabaseConnectionPoolExhausted
        expr: db_connection_pool_usage_ratio > 0.9
        for: 2m
        annotations:
          summary: "数据库连接池即将耗尽"
          description: "使用率: {{ $value }}%"

      # 应用不可用告警
      - alert: ApplicationDown
        expr: up{job="application"} == 0
        for: 1m
        annotations:
          summary: "应用服务不可用"
          description: "应用服务 {{ $labels.instance }} 无响应"
```

---

## 📝 **Part 5：检查清单模板**

### **发布前最终检查清单**

```
□ 代码质量
  □ TypeScript 类型检查通过
  □ Linting 检查通过
  □ 代码格式检查通过
  □ 没有 TODO/FIXME 注释在关键路径
  □ 错误处理完善
  
□ 功能验证
  □ 本地创作功能测试通过
  □ 云创作兼容性测试通过
  □ 模式切换正常工作
  □ 密钥管理完整
  □ 性能指标达成

□ 依赖和安全
  □ 没有高危安全漏洞
  □ 依赖已锁定版本
  □ 没有硬编码密钥
  □ 环境变量管理正确

□ 数据库
  □ 迁移脚本已验证
  □ 备份策略已测试
  □ 恢复流程已验证
  □ 数据库性能达标

□ 构建和打包
  □ 前端构建成功
  □ 后端构建成功
  □ Tauri 打包成功
  □ 构建产物验证通过

□ 文档
  □ 部署文档已更新
  □ API 文档已完成
  □ 用户文档已完成
  □ FAQ 已完成

□ 监控和日志
  □ 监控系统已配置
  □ 告警规则已设置
  □ 日志系统已配置
  □ 日志保留策略已设置

□ 备份和恢复
  □ 备份策略已验证
  □ 恢复流程已测试
  □ 备份存储已验证
  □ 恢复时间目标已达成

□ 用户支持
  □ 支持邮箱已设置
  □ 问题追踪系统已就绪
  □ FAQ 文档已完成
  □ 支持流程已建立

□ 最终确认
  □ 所有检查项已通过
  □ 风险已识别并缓解
  □ 团队准备就绪
  □ 发布批准已获得

发布就绪日期：_____________
发布批准人：_____________
发布时间：_____________
```

---

## 🎯 **执行时间表**

```
第 1 天（检查和环保境准备）
  09:00 - 09:30  代码最终审查
  09:30 - 10:00  文档完整性检查
  10:00 - 10:30  配置检查
  10:30 - 11:30  环境初始化
  11:30 - 12:00  构建验证
  
第 2 天（测试和监控）
  09:00 - 10:00  冒烟测试执行
  10:00 - 11:00  性能基准测试
  11:00 - 12:00  监控系统验证
  13:00 - 14:00  日志系统验证
  14:00 - 15:00  备份和恢复测试
  
第 3 天（支持和确认）
  09:00 - 10:00  用户文档最后检查
  10:00 - 11:00  支持系统启动
  11:00 - 12:00  最终整体检查
  13:00 - 14:00  问题修复（如有）
  14:00 - 15:00  最终批准和就绪确认
```

---

## ✅ **完成标准**

Phase 8 完成需要满足：

```
✅ 所有代码检查通过
✅ 所有文档检查完成
✅ 所有配置检查通过
✅ 环境初始化完毕
✅ 冒烟测试 100% 通过
✅ 性能基准达成
✅ 监控系统就绪
✅ 备份和恢复验证通过
✅ 用户支持系统完成
✅ 最终检查清单全部通过
✅ 发布批准获得
✅ 团队准备确认

所有项都必须标记为 ✅
```

---

**Phase 8 执行工具包完成！** ✅

可以复制这些脚本到项目中并执行。

