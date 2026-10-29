import type { Snapshot } from "valtio/vanilla";
import type {
  BackendRequest,
  BackendTrigger,
  ExecutionTaskDetails,
  Initiator,
  LocalSoftwarePrepared,
} from "../self-service/types";
import { boundedText, localActionLabels } from "../software/presentation";

const plain = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === "object" && !Array.isArray(value);
function protectedSchema(value: unknown): boolean {
  return Array.isArray(value)
    ? value.some(protectedSchema)
    : plain(value) &&
        (value.writeOnly === true ||
          value.format === "password" ||
          Object.values(value).some(protectedSchema));
}
function policyParameter(
  record: Snapshot<BackendRequest>,
  name: string,
): string {
  const action = record.action;
  if (
    action.kind !== "policy" ||
    action.task.kind !== "script" ||
    protectedSchema(action.task.parameterSchema)
  )
    return "受保护输入不回显";
  const source = action.task.parameterSources[name];
  const value =
    source?.kind === "fixed" ? source.value : action.choice.parameters[name];
  return value === undefined
    ? "当前回执未提供原值"
    : value === null
      ? "空值"
      : ["string", "number", "boolean"].includes(typeof value)
        ? String(value).slice(0, 4096)
        : "复杂输入由原服务保管";
}
/** Only service-owned, value-only summaries are rendered here; these facts grant no permission. */
export function OriginFacts({
  origin,
}: {
  origin: Snapshot<Initiator | BackendTrigger>;
}) {
  const trigger = origin.kind === "backend" ? origin.trigger : origin;
  return (
    <>
      <dt>发起来源</dt>
      <dd>
        {trigger.kind === "ai"
          ? "AI 具体动作"
          : trigger.kind === "human"
            ? "人工操作"
            : "后台策略"}
      </dd>
      {"osSession" in trigger ? (
        <>
          <dt>发起设备</dt>
          <dd className="rss-reference">{trigger.osSession.device}</dd>
          <dt>发起本机用户</dt>
          <dd>
            {trigger.osSession.account.platform} ·{" "}
            {trigger.osSession.account.subject}
          </dd>
          <dt>本机会话</dt>
          <dd className="rss-reference">{trigger.osSession.session}</dd>
        </>
      ) : (
        <>
          <dt>发起本机用户</dt>
          <dd>后台来源未提供本机用户，不按当前登录用户推断。</dd>
        </>
      )}
      {trigger.kind === "ai" && (
        <>
          <dt>原 AI 会话</dt>
          <dd className="rss-reference">{trigger.conversation}</dd>
          <dt>原工具调用</dt>
          <dd className="rss-reference">{trigger.toolCall}</dd>
          <dt>连接配置版本</dt>
          <dd>
            {trigger.config.id} · {trigger.config.revision}
          </dd>
        </>
      )}
    </>
  );
}
export function SoftwareFacts({
  prepared,
}: {
  prepared: Snapshot<LocalSoftwarePrepared>;
}) {
  return (
    <div className="rss-preparation-facts">
      <div className="rss-preparation-object">
        <span className="rss-preparation-glyph">
          {localActionLabels[prepared.choice.action].slice(0, 1)}
        </span>
        <div>
          <h3>{prepared.name}</h3>
          <p>本机原安装 · {localActionLabels[prepared.choice.action]}</p>
        </div>
      </div>
      <dl className="rss-task-facts">
        <dt>原请求</dt>
        <dd className="rss-reference">{prepared.request}</dd>
        <dt>原安装引用</dt>
        <dd className="rss-reference">
          {prepared.choice.software.installation}
        </dd>
        <dt>固定目标版本</dt>
        <dd>{prepared.targetVersion ?? "此操作未提供目标版本"}</dd>
        <OriginFacts origin={prepared.initiator} />
        <dt>执行身份类别</dt>
        <dd>
          {prepared.identity === "system" ? "系统执行身份" : "本机用户执行身份"}
          ，不代表授予管理员权限。
        </dd>
        <dt>首次启动期限</dt>
        <dd>{new Date(prepared.startBeforeUnixMs).toLocaleString()}</dd>
        <dt>动作摘要</dt>
        <dd>
          {prepared.choice.action === "uninstall"
            ? "卸载会修改本机安装状态；数据保留由软件卸载器决定。"
            : prepared.choice.action === "launch"
              ? "打开原安装对应的应用。"
              : prepared.choice.action === "detect"
                ? "读取原安装的实际状态。"
                : "使用服务冻结的候选版本；当前服务能力仍会在提交时核对。"}
        </dd>
      </dl>
    </div>
  );
}
export function PolicyFacts({
  record,
  parameterSummary,
}: {
  record: Snapshot<BackendRequest>;
  parameterSummary?: Snapshot<Record<string, string>>;
}) {
  const action = record.action;
  if (action.kind !== "policy" && action.kind !== "offered")
    return (
      <p className="rss-muted">
        此准备由原业务来源持有；具体范围以原任务事实为准。
      </p>
    );
  const policy = action.kind === "policy" ? action : null,
    offer = action.offer,
    title = policy
      ? boundedText(policy.presentation.displayName, 120) ||
        policy.definition.id
      : offer?.presentation.title,
    expires =
      policy?.expiresAtUnixMs ?? (offer ? offer.expiresAt * 1000 : null);
  return (
    <div className="rss-preparation-facts">
      <div className="rss-preparation-object">
        <span className="rss-preparation-glyph">核</span>
        <div>
          <h3>{title}</h3>
          <p>服务已冻结的原准备 · 当前记录版本 {record.revision}</p>
        </div>
      </div>
      <dl className="rss-task-facts">
        <OriginFacts origin={record.trigger} />
        {policy && (
          <>
            <dt>发布项目</dt>
            <dd className="rss-reference">{policy.definition.id}</dd>
            <dt>固定发布版本</dt>
            <dd className="rss-reference">{policy.definition.revision}</dd>
            <dt>策略配置版本</dt>
            <dd>{policy.policyRevision}</dd>
            <dt>授权范围</dt>
            <dd>
              {policy.access.kind === "device" ? "设备范围" : "原企业用户绑定"}
              ，每次操作仍由原服务核对。
            </dd>
            <dt>准备类型</dt>
            <dd>{policy.task.kind}</dd>
            {policy.task.kind !== "enrollment" && (
              <>
                <dt>固定资源</dt>
                <dd className="rss-reference">
                  {policy.task.resource.id} · {policy.task.resource.version} ·{" "}
                  {policy.task.resource.variant}
                </dd>
                <dt>执行身份类别</dt>
                <dd>
                  {policy.task.runAs === "system"
                    ? "系统执行身份"
                    : "本机用户执行身份"}
                </dd>
              </>
            )}
          </>
        )}
        {!policy && offer?.presentation.summary.kind === "script" && (
          <>
            <dt>准备类型</dt>
            <dd>脚本</dd>
            <dt>执行身份类别</dt>
            <dd>
              {offer.presentation.summary.identity === "system"
                ? "系统执行身份"
                : "本机用户执行身份"}
            </dd>
          </>
        )}
        {expires != null && (
          <>
            <dt>准备有效期限</dt>
            <dd>{new Date(expires).toLocaleString()}</dd>
          </>
        )}
        <dt>企业准入</dt>
        <dd>
          由原执行服务核对当前发布、主体、风险与权限；页面确认不能覆盖拒绝。
        </dd>
        <dt>OS 权限</dt>
        <dd>实际执行仍受本机用户、系统授权和运行条件限制。</dd>
      </dl>
      {offer?.presentation.summary.kind === "software" && (
        <div className="rss-prepared-steps">
          <h4>固定软件动作 · {offer.presentation.summary.intent}</h4>
          {offer.presentation.summary.steps.map((step, index) => (
            <p key={index}>
              {step.package} · {step.version} ·{" "}
              {step.identity === "system" ? "系统身份" : "用户身份"}
            </p>
          ))}
        </div>
      )}
      {parameterSummary && Object.keys(parameterSummary).length > 0 && (
        <div>
          <h4>参数核对</h4>
          <dl className="rss-task-facts">
            {Object.entries(parameterSummary).map(([name, kind]) => (
              <div className="rss-fact-pair" key={name}>
                <dt>{name}</dt>
                <dd>{kind} · 原值由执行服务保管</dd>
              </div>
            ))}
          </dl>
        </div>
      )}
      {policy?.task.kind === "script" && !parameterSummary && (
        <div>
          <h4>参数核对</h4>
          <dl className="rss-task-facts">
            {Object.entries(policy.task.parameterSources).map(
              ([name, source]) => (
                <div className="rss-fact-pair" key={name}>
                  <dt>{name}</dt>
                  <dd>
                    {source.kind === "fixed"
                      ? "由可信策略固定"
                      : "原请求提交的输入"}
                    ：{policyParameter(record, name)}；已冻结。
                  </dd>
                </div>
              ),
            )}
          </dl>
        </div>
      )}
    </div>
  );
}
export function ExecutionFacts({
  details,
}: {
  details: Snapshot<ExecutionTaskDetails>;
}) {
  const action = details.action;
  return (
    <div className="rss-preparation-facts">
      <dl className="rss-task-facts">
        <OriginFacts origin={action.initiator} />
        <dt>目标设备</dt>
        <dd className="rss-reference">{action.target.device}</dd>
        <dt>目标范围</dt>
        <dd>
          {action.target.platform} ·{" "}
          {action.target.scope.kind === "device"
            ? "设备"
            : `本机用户 ${action.target.scope.account.subject}`}
        </dd>
        <dt>执行主体</dt>
        <dd className="rss-reference">{action.actor}</dd>
        <dt>实际执行身份</dt>
        <dd>
          {action.runAs.kind === "system"
            ? "系统身份"
            : `${action.runAs.account.platform} · ${action.runAs.account.subject}`}
        </dd>
        <dt>具体动作</dt>
        <dd>{action.operation.action}</dd>
        <dt>固定资源</dt>
        <dd className="rss-reference">
          {action.operation.resource.id} · {action.operation.resource.revision}
        </dd>
        <dt>策略版本</dt>
        <dd className="rss-reference">
          {action.policy.id} · {action.policy.revision}
        </dd>
        <dt>首次执行期限</dt>
        <dd>{new Date(action.validity.expiresAtUnixMs).toLocaleString()}</dd>
        <dt>运行来源</dt>
        <dd>
          {details.status.mode === "test"
            ? "显式测试执行器；不能作为真实设备效果证明"
            : "生产执行服务"}
        </dd>
        <dt>权限约束</dt>
        <dd>
          {action.access.kind === "restricted"
            ? `声明读取 ${action.access.readPathCount} 项、写入 ${action.access.writePathCount} 项；${action.access.networkDenied ? "禁止网络" : `允许列表 ${action.access.networkDestinationCount} 项`}；${action.access.requireSandbox ? "要求隔离" : "以执行身份约束"}`
            : "由原生 OS 执行身份和权限约束"}
        </dd>
      </dl>
      {action.execution.kind === "softwareProgram" && (
        <div className="rss-prepared-steps">
          <h4>原软件计划 · {action.execution.intent}</h4>
          {action.execution.steps.map((step, index) => (
            <p key={index}>
              {step.package} · {step.version} · {step.adapter} ·{" "}
              {step.runAs.kind === "system" ? "系统身份" : "指定用户身份"}
            </p>
          ))}
        </div>
      )}
    </div>
  );
}
