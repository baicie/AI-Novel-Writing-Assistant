# Docker 自托管部署边界

## 背景

桌面版适合本机写作；源码 `pnpm dev` 适合开发。把工作台放到一台服务器上、用浏览器打开，需要把前端静态资源、API、数据库和可选的知识库检索放进同一套编排。

仓库里已经有 `Dockerfile.api` 和 `Dockerfile.web`，但原先缺少：

- 一次启动前后端和数据库的 Compose 编排
- 浏览器只访问一个端口时的同源 `/api` 反向代理
- 容器启动时自动应用 PostgreSQL 迁移

如果继续要求用户分别暴露 API 端口、构建时写死 `VITE_API_BASE_URL`、再手动跑迁移，新手很难在服务器上跑通。

## 决策

- 服务器部署默认使用 **PostgreSQL**，不用容器里的 SQLite。SQLite 仍是桌面版和本机开发的默认；多容器、重启和数据卷更适合独立数据库进程。
- 浏览器只访问 Web 端口。Nginx 把 `/api/` 反向代理到 API 容器，前端生产包继续使用相对路径 `/api`，不在镜像里写死服务器地址。
- API 端口不对外发布。数据库端口也不对外发布。只有 Web 端口映射到宿主机。
- 容器启动时执行 `prisma migrate deploy`。这是 PostgreSQL 的结构来源；桌面/SQLite 的 `ensureRuntimeDatabaseReady` 不会在这个部署路径里跑。
- 知识库检索默认关闭。Qdrant 放在 Compose 的 `rag` profile 里，需要时再启用，避免第一次部署被检索依赖挡住。
- 模型密钥优先在网页「模型与厂商」里保存。Compose 环境文件只放部署连接参数、数据库密码和网页访问密码。
- 设置 `ACCESS_PASSWORD` 后，浏览器和除健康检查、登录、已签名外部回调以外的接口都要先持有访问会话。桌面版和本地开发不设置该密码，因此不会出现登录页。这是单人访问门，不是多用户账号。

## 当前规则

- 入口是仓库根目录 `docker-compose.yml`。本机只起 Qdrant 时，仍可使用 `infra/docker-compose.qdrant.yml`。
- 环境模板是 `infra/docker/.env.example`，复制到仓库根目录 `.env` 后修改密码。数据库连接串由 Compose 根据 `POSTGRES_*` 拼出，避免两处密码不一致。
- 前端镜像构建时保持 `VITE_API_BASE_URL` 为空。改成绝对地址会让用户换域名或端口后必须重建前端。
- Nginx 必须关闭 `/api/` 的缓冲，并把读超时放到小时级。Creative Hub、AI 实况和章节生成走长连接或 SSE，缓冲会把流式输出卡住。
- 服务器部署应设置 `ACCESS_PASSWORD`。登录态放在 HttpOnly Cookie 里，浏览器只访问 Web 端口时会自动带上。改密码会使已打开的页面重新要求输入。
- 钉钉和企业微信的已签名回调不走访问密码，它们继续用各自的签名校验。不要为了登录墙把这些地址改成必须带 Cookie。
- 访问密码不能代替网络边界。公网机器仍应配合防火墙或 VPN；示例密码必须改掉。
- 数据库密码不要包含 `@:/%#` 等 URL 特殊字符，否则 `DATABASE_URL` 会解析失败。
- 升级时先备份 Postgres 数据卷，再拉新代码构建。不要对正在使用的数据卷执行 `docker compose down -v`。

## 示例

推荐做法：

- 在服务器上 `cp infra/docker/.env.example .env`，改密码后执行 `docker compose up -d --build`，用浏览器打开 `http://服务器:8080`。
- 需要知识库时再执行 `docker compose --profile rag up -d`，并在网页向量设置里配置 Embedding。
- 排查接口问题时看 `docker compose logs -f api`，不要把 API 端口临时映射到公网。

不推荐或禁止做法：

- 为了“先看效果”把 Postgres 端口发到 `0.0.0.0:5432`。
- 在前端镜像里写死 `http://127.0.0.1:3000/api`。服务器上的浏览器会打到用户自己的电脑。
- 把 `ALLOW_LAN=true` 留给 Docker 部署。容器网络里只应让 Nginx 访问 API。
- 用 `migrate reset` 或删除 named volume 当作升级手段。

## 失败模式

- 打开页面但接口 502：先看 API 是否 healthy，再看 `prisma migrate deploy` 是否在等数据库。密码和 `POSTGRES_*` 不一致时迁移会一直失败。
- 页面能开、模型调用一直转圈：检查 Nginx 是否缓冲了 `/api/`，以及 `proxy_read_timeout` 是否过短。
- 知识库无法索引：确认是否使用了 `--profile rag`，以及网页里的 Qdrant 地址是否指向 `http://qdrant:6333`（容器内）而不是宿主机 `127.0.0.1`。
- 重启后小说消失：通常是用了 `-v` 删掉数据卷，或把 Postgres 卷换成了一次性容器文件系统。
- 正确密码仍回到登录页：确认请求打到 Web 容器的 `/api/`，Cookie 是 `HttpOnly` 且没有在 HTTP 页面上被标成 `Secure`。改 `ACCESS_PASSWORD` 后旧 Cookie 会失效，这是预期行为。
- 外部渠道按钮失效并返回需要访问密码：回调路径被登录墙拦住了。签名回调必须留在 `/api/auto-director/channel-callbacks/`，不能要求浏览器 Cookie。

## 相关模块

- `docker-compose.yml`
- `Dockerfile.api`
- `Dockerfile.web`
- `infra/docker/api-entrypoint.sh`
- `infra/nginx/ai-novel-web.conf`
- `server/src/platform/access/accessSession.ts`
- `server/src/routes/access.ts`
- `client/src/components/layout/AccessGate.tsx`
- `server/src/config/database.ts`
- `client/src/lib/constants.ts`

## 来源文档

- [配置项归属与可见性规范](./configuration-conventions.md)
- [安装与准备](../../public/installation.md)
