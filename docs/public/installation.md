# 安装与准备

这篇文档帮助你在 Windows 上准备 AI 小说创作工作台（Biz Novel Studio），并确认模型、存储和知识库能力是否可用。

## 推荐安装方式

普通用户优先使用 GitHub Releases 页面提供的桌面版：

1. 打开 [GitHub Releases](https://github.com/ExplosiveCoderflome/AI-Novel-Writing-Assistant/releases/latest)。
2. 下载 Windows 安装包或 portable 包。
3. 运行应用，进入系统设置。
4. 配置至少一个可用模型供应商。
5. 创建一本测试小说，跑通新手上路。

安装版适合长期使用；portable 版适合临时试用或放在独立目录中运行。

## 首次启动前准备

你需要准备：

- 一个可用的 LLM API Key。
- 供应商的 Base URL 或官方兼容接口。
- 一个适合规划、分析和正文生成的模型。
- 稳定网络连接。
- 足够的本地磁盘空间，用于保存数据库、日志、素材和生成结果。

如果你还没有确定模型，推荐先使用 DeepSeek Flash 跑通主链，再到模型路由里为正文、审核、拆书等任务细分模型。

## 基础配置顺序

推荐按这个顺序完成：

1. 打开系统设置。
2. 填写模型供应商、API Key、Base URL 和默认模型。
3. 使用连接测试确认模型可以返回结果。
4. 创建一本测试小说。
5. 输入一句简单灵感，例如“一个新人作者在灵感市场里寻找失落故事”。
6. 跟随新手上路或自动导演推进到第一章。

先跑通最短路径，再配置知识库、写法引擎和高级模型路由，会更容易定位问题。

## 数据保存位置

桌面版默认把小说、任务状态、配置和本地数据库保存在应用数据目录中。你可以在故障排查文档中查看日志和数据备份建议。

重要项目建议定期备份。备份时优先保留：

- 应用数据库文件。
- 小说导出文件。
- 角色、世界、知识库和写法资产。
- 任务日志或错误截图。

不要在没有备份的情况下删除数据库文件或重置数据。

## Qdrant 是否必须

Qdrant 用于向量检索和知识库召回。它不是创建小说和生成第一章的硬性前置条件，但会影响资料检索、拆书沉淀和长篇一致性。

推荐策略：

- 只想先体验主链：先不配置 Qdrant。
- 需要知识库检索：配置 Qdrant 并测试索引。
- 需要大量参考资料：优先保证 Qdrant 稳定运行。

如果知识库不命中，先检查文档是否完成索引，再检查检索设置和 Qdrant 连接。

## 源码运行

开发者可以从源码运行站点、客户端和服务端。请先确认 Node.js 和 pnpm 版本满足仓库要求，再安装依赖。

常用命令：

```bash
pnpm install
pnpm dev
pnpm build
```

只预览公开站点：

```bash
pnpm --filter @ai-novel/site dev
```

源码运行适合开发和调试；普通写作使用优先选择桌面版。

## 用 Docker 部署到服务器

把工作台放到一台已安装 Docker 的服务器上，可以用浏览器直接打开页面。不需要在服务器上安装 Node.js。

1. 安装 [Docker Engine](https://docs.docker.com/engine/install/) 和 Compose 插件。
2. 克隆仓库并复制环境模板：

```bash
git clone https://github.com/ExplosiveCoderflome/AI-Novel-Writing-Assistant.git
cd AI-Novel-Writing-Assistant
cp infra/docker/.env.example .env
```

3. 编辑 `.env`，把 `POSTGRES_PASSWORD` 和 `ACCESS_PASSWORD` 改成你自己的密码。数据库密码不要使用 `@:/%#` 等特殊字符。`ACCESS_PASSWORD` 是打开网页时要输入的访问密码；留空则不启用登录页。
4. 构建并启动：

```bash
docker compose up -d --build
```

5. 浏览器打开 `http://服务器IP:8080`，输入访问密码。进入后到「模型与厂商」填写 API Key 并做连接测试。页面右上角可以退出，这台浏览器需要重新输入密码。

默认不开启知识库检索。需要检索时：

```bash
docker compose --profile rag up -d
```

然后在网页的向量设置里把 Qdrant 地址设为 `http://qdrant:6333`。

这套部署把数据库和生成图片放在 Docker 数据卷里。升级前先备份；不要使用 `docker compose down -v`，那会删除小说数据。访问密码只保护这一个部署入口，公网暴露时仍请用防火墙或 VPN 限制谁能连到服务器。

## 安装后建议

安装完成后，先做三件事：

1. 在系统设置里完成模型连接测试。
2. 用一条简单灵感创建测试小说。
3. 在任务中心确认长任务能正常显示进度和结果。

这三步能覆盖模型、数据库和任务状态。如果任一步失败，先阅读《常见问题》和《故障排查》。
