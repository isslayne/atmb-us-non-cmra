# atmb-us-non-cmra

抓取 [Anytime Mailbox](https://www.anytimemailbox.com/) 美国地址，通过 [Smarty](https://www.smarty.com/) 检查 CMRA / Residential Delivery Indicator，并显示 USPS 地址查询结果。代码、Actions 构建和结果更新均使用 `dev` 分支。

## GitHub Actions

在 [Actions → Update US addresses](https://github.com/isslayne/atmb-us-non-cmra/actions/workflows/update-addresses.yml) 点击 **Run workflow**，选择 `dev`：

| 配置 | 默认值 | 说明 |
| --- | --- | --- |
| `address_scope` | `tax-free` | `tax-free`：AK、DE、MT、NH、OR；`all`：美国各州、DC 及属地的可用地址 |
| `usps_enabled` | `true` | 是否查询 USPS |
| `usps_interval_ms` | `1000` | USPS 请求间隔，单位毫秒 |
| `max_addresses` | `0` | 0 表示完整运行；正整数仅抽取前 N 个地址进行测试，不提交结果 |
| `credentials_secret` | `CREDENTIALS` | 存放多账号凭据的仓库 Secret **名称**，可留空使用默认名称 |
| `smarty_auth_id_secret` | `SMARTY_AUTH_ID` | 存放 auth ID 的仓库 Secret **名称** |
| `smarty_auth_token_secret` | `SMARTY_AUTH_TOKEN` | 存放 auth token 的仓库 Secret **名称** |

在 [Settings → Secrets and variables → Actions](https://github.com/isslayne/atmb-us-non-cmra/settings/secrets/actions) 配置 `SMARTY_AUTH_ID` 和 `SMARTY_AUTH_TOKEN`，或者配置优先级更高的 `CREDENTIALS`，格式为 `API_ID1=API_TOKEN1,API_ID2=API_TOKEN2`。原 README 中的 `CRENDENTIALS` 拼写已修正为 **`CREDENTIALS`**。

手动运行时可填写其他 Secret 名称切换账号。**不要把真实 token 填入 Run workflow 的普通输入框**：这些输入不是密码字段。凭据仅通过运行进程的环境变量传入，不保存到代码、CSV 或日志。

定时任务每周一 **02:17 UTC（北京时间 / 新加坡时间 10:17）**运行，默认分析免税州并查询 USPS。可修改 `.github/workflows/update-addresses.yml` 的 cron 调整频率。Smarty 的套餐与额度以账号实际配置为准；查询会消耗额度。多账号仅在服务返回 HTTP 402 时切换，不再假定每个账号固定有 1000 次额度。

GitHub 要求定时工作流位于默认分支，因此此仓库应将 **`dev` 设为默认分支**。工作流始终 checkout `dev`，完整且 Smarty / USPS 无服务错误的运行会自动提交结果到 `dev`。并发更新会排队；结果未变化时不生成提交。

## 结果

- `result/tax-free/mailboxes.csv`：免税州中 Smarty 明确标记 `CMRA=N` 的地址，住宅优先。
- `result/tax-free/checks.csv`：免税州全部地址及两种查询的结果，包括 CMRA、未匹配和服务错误。
- `result/all/`：全部美国地址模式的同类文件，与免税州结果分别保存。
- `summary.md`：运行统计，同时显示在 Actions Summary 中。
- Actions Artifacts：CSV 和统计，保留 30 天；失败运行有已生成结果时也会上传供排查。限量测试结果只在 `result/smoke/` 和 Artifact 中。
- 原 `result/mailboxes.csv` 是历史结果，不再更新；请使用上述分目录输出。

ATMB 部分详情链接存在缓存的目录页重定向。程序遇到错误或非详情页时会带刷新参数重试。仍无法取得详情时，使用州列表公开的街道、城市、州、ZIP 继续查询 Smarty / USPS，并以 `detail_status=listing_fallback` 标记；详情正常时为 `fetched`。不会再因为详情页不可用而跳过有效的列表地址。`listing_fallback` 的单元号未经详情页确认，查询结果只对应 CSV 中实际提交的地址；需要已确认单元号时请进一步筛选 `detail_status=fetched`。

CSV 包含原地址、独立的 `street2`、价格、链接、Smarty `rdi` / `CMRA` / `smarty_status`，以及 USPS 的 `usps_status`、标准化地址、ZIP5 / ZIP4、DPV confirmation、CMRA、business、carrier route 和完整响应 JSON（`usps_raw`）。USPS 实际响应的 CMRA 字段名为 `cmar`，程序读取后写入 `usps_cmra`。USPS 未返回的字段留空，不根据其他字段推断。多个匹配记录以 `multiple_matches` 标记，完整候选保留在 JSON 中。

USPS 默认通过独立 Chrome 会话打开正常查询页面、填写表单，由 USPS 网页自身脚本发出请求；不读取用户浏览器的 Cookie 或配置。Actions 使用已实测可用的 macOS Runner + 有界面 Chrome。直接 HTTP POST 缺少网站初始化的会话，请求可能被重定向到错误页，即使相同地址在浏览器可查询。网页使用 `POST https://tools.usps.com/tools/app/ziplookup/zipByAddress`，无鉴权，按 `application/x-www-form-urlencoded` 提交 `companyName`、`address1`、`address2`、`city`、`state`、`urbanCode`、`zip` 等字段。`state` 始终是两位缩写；实际套房 / 单元号保留在 `address2`，未分配的 `YOUR NAME` / `MAILBOX` 占位符不发送。

USPS 网站接口可能返回重定向、限流或非 JSON 页面。程序记录 `http_XXX` / `invalid_json` / `network_error` 等状态；连续三次服务错误后，其余地址标记 `skipped_after_service_errors`，避免持续请求不可用服务。USPS 不可用时仍生成诊断 CSV，但工作流失败并阻止覆盖已发布结果；Summary 会显示不可用数量。**USPS 查询匹配不代表非 CMRA；失败或未知也不代表通过。** Smarty 返回 `http_402_subscription_required` 表示账号缺少此 API 的有效订阅，需要在 Smarty 账号中开通 / 恢复订阅，或在 Actions 中选择其他凭据 Secret 后重跑。Smarty 服务错误时保留诊断文件并让 Actions 失败，不发布部分结果。

这里的“免税州”指没有州级一般销售税的五州，不代表所有交易都免税；例如 Alaska 可能有地方销售税。[税制范围说明](https://taxfoundation.org/data/all/state/sales-tax-rates-2025/)

## 本地运行

安装 Rust stable 和 Node.js 24，并安装浏览器依赖：

```sh
npm ci
npx playwright install chrome
export USPS_CHANNEL=chrome
export USPS_HEADLESS=false
```

Linux 有界面模式需要显示环境（例如 Xvfb）；`USPS_ENABLED=false` 时不需要 Node / Chrome。可用 `npm run test:usps-live` 单独验证 USPS，或手动运行 **Verify upstream address queries** 工作流。旧的 `USPS_BACKEND=http` 仅供排查，不是 Actions 默认方式。

本地默认使用 reqwest；Actions 使用 Runner 自带的 curl 访问 ATMB，以兼容源站的 HTTP 客户端限制。也可在本地设置 `ATMB_HTTP_BACKEND=curl`（需安装 curl）。复制 `.env.example` 为 `.env`，填入自己的凭据（`.env` 已忽略），然后执行：

```sh
set -a
. ./.env
set +a
cargo run --release --locked
```

也可以自行导出 `CREDENTIALS` 或 `SMARTY_AUTH_ID` / `SMARTY_AUTH_TOKEN` 环境变量。程序不会自动读取 `.env`。

```sh
# 全美国地址
ADDRESS_SCOPE=all cargo run --release --locked
# 默认免税州，小规模校验；不会覆盖正式结果
MAX_ADDRESSES=2 cargo run --release --locked
# 检查
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```
