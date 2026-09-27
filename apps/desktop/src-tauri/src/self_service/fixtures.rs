use super::model::*;
use execution_contract::{
    DeviceId, ExecutionLimits, FrozenExecution, Id, Initiator, OsAccountRef, OsSessionRef, Platform,
};
use serde_json::json;
use service_catalog::{CatalogLimits, FrozenCatalog, ParameterLimits, SelectedOperation};
use sha2::{Digest as _, Sha256};

pub fn os_session() -> OsSessionRef {
    OsSessionRef {
        device: DeviceId::new("fixture-device").unwrap(),
        account: OsAccountRef {
            platform: Platform::Macos,
            subject: Id::new("fixture-user").unwrap(),
        },
        session: Id::new("fixture-session").unwrap(),
    }
}
pub fn human() -> Initiator {
    Initiator::Human {
        os_session: os_session(),
    }
}

pub const TARGET: &str = "macOS arm64 · 测试设备 fixture-device · 测试用户 fixture-user";
pub const ARTIFACT: &[u8] = b"RSS desktop fixed test artifact. Not executable.\n";
pub const CATALOG_LIMITS: CatalogLimits = CatalogLimits {
    max_bytes: 65536,
    max_depth: 16,
    max_nodes: 4096,
    max_string_bytes: 4096,
    max_collection_items: 128,
};
pub const PARAMETERS: ParameterLimits = ParameterLimits {
    max_bytes: 4096,
    max_string_bytes: 1024,
    max_parameters: 16,
};
pub const INPUT_LIMITS: ExecutionLimits = ExecutionLimits {
    max_input_bytes: 16384,
    max_depth: 16,
    max_nodes: 2048,
    max_string_bytes: 4096,
    max_collection_items: 128,
    max_timeout_ms: 60_000,
    max_output_bytes: 4096,
    max_stdin_bytes: 1024,
    max_attempts: 1,
};
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn catalog(_now: u64) -> Result<FrozenCatalog> {
    let mut items = Vec::new();
    for (id, kind, name, description) in [
        (
            "office",
            "software",
            "办公套件",
            "确认本次测试动作后执行；不会安装软件。",
        ),
        (
            "diagnostics",
            "tool",
            "网络诊断",
            "只读测试动作；不会发送网络请求。",
        ),
        (
            "long-running",
            "tool",
            "持续执行示例",
            "保持测试执行直至取消；不安排维护窗口。",
        ),
        (
            "parameter-check",
            "tool",
            "参数校验示例",
            "按目录规则校验输入后执行；更改输入需新建请求。",
        ),
        (
            "unknown",
            "tool",
            "未知效果示例",
            "固定未知结果，必须保持待核实，不能重跑掩盖。",
        ),
        (
            "blocked",
            "software",
            "受限软件",
            "固定权限拒绝样本，无法申请或提交。",
        ),
        (
            "unsupported",
            "software",
            "不适用软件",
            "固定目标不适用样本。",
        ),
        (
            "withdrawn",
            "tool",
            "已下架工具",
            "历史目录记录，仅供浏览。",
        ),
    ] {
        let parameters = match id {
            "diagnostics" | "parameter-check" => json!({
                "host":{"title":"诊断目标","description":"纯测试文本，不发起网络连接","required":true,"rule":{"type":"string","minLength":3,"maxLength":100}},
                "count":{"title":"采样次数","description":"留空使用目录默认值","required":false,"rule":{"type":"integer","minimum":1,"maximum":5,"default":3}},
                "detail":{"title":"包含详细信息","description":"仅模拟诊断说明","required":false,"rule":{"type":"boolean","default":false}},
                "credential":{"title":"测试凭据引用","description":"仅填写引用 ID 和版本，不输入密码正文","required":false,"rule":{"type":"secretReference"}}
            }),
            "office" => {
                json!({"edition":{"title":"软件版本","description":"测试版本选择","required":false,"rule":{"type":"string","minLength":1,"maxLength":20,"choices":["standard","professional"],"default":"standard"}}})
            }
            _ => json!({}),
        };
        items.push(json!({
            "id":id,"kind":kind,"name":name,"description":description,"category":if kind == "software" {"软件"} else {"工具"},
            "aiDiscoverable":true,"distributionHint":if id == "office" {"request"} else {"optional"},
            "state":if id == "withdrawn" {"withdrawn"} else {"listed"},
            "operations":[{"id":"test","action":if kind == "software" {"install"} else {"diagnose"},
            "resource":{"reference":{"id":format!("fixture-{id}"),"revision":"r1"},"versionDigest":digest(format!("fixture resource definition v1: {id}").as_bytes()),"selector":{"platform":"macos","architecture":"aarch64","key":"fixture"}},
            "parameters":parameters,"requirements":{"capabilities":["fixture-only"],"runAs":"targetUser","interaction":"userSession","evidence":["fixture-result"]}}]
        }));
    }
    let bytes=serde_json::to_vec(&json!({"schemaVersion":1,"authority":{"kind":"test","id":"desktop-fixture"},"identity":{"id":"self-service","revision":"r1"},"expiresAtUnixMs":4_102_444_800_000_u64,"items":items})).map_err(|_|error("fixture","测试目录编码失败"))?;
    Ok(service_catalog::decode_catalog(&bytes, &CATALOG_LIMITS)?)
}

pub fn freeze(
    selected: &SelectedOperation,
    request_id: &execution_contract::RequestId,
    now: u64,
    initiator: &execution_contract::Initiator,
    actor: &execution_contract::ActorId,
) -> Result<FrozenExecution> {
    let operation = selected.operation();
    let account = json!({"platform":"macos","subject":"fixture-user"});
    let artifact = json!({"resource":operation.resource.reference,"sha256":digest(ARTIFACT)});
    let spec = json!({
        "schemaVersion":4,"execution":{"kind":"process"},
        "request":{"schemaVersion":1,"requestId":request_id,"authority":{"kind":"test","id":"desktop-fixture"},"actor":actor,"initiator":initiator,"delegation":null,
        "target":{"device":"fixture-device","platform":"macos","scope":{"kind":"user","account":account}},"operation":{"action":operation.action,"resource":operation.resource.reference},"parameters":selected.parameters()},
        "launch":{"artifact":artifact,"interpreter":{"artifact":{"resource":{"id":"fixture-interpreter","revision":"r1"},"sha256":digest(b"fixed interpreter marker; no interpreter exists")},"profile":{"id":"fixture-only","revision":"r1"}},"argv":[{"kind":"artifactPath"}],"artifactEncoding":"utf8","stdin":{"kind":"closed"},"output":{"format":{"kind":"text"},"stdout":"utf8","stderr":"utf8"},"cwd":"/s1-fixture","env":{}},
        "runAs":{"kind":"user","account":account},"constraints":{"kind":"restricted","network":{"kind":"denied"},"readPaths":[],"writePaths":[],"allowChildProcesses":false,"requireSandbox":true},"budget":{"totalTimeoutMs":60_000,"totalOutputBytes":4096,"maxAttempts":1},"validity":{"notBeforeUnixMs":now,"expiresAtUnixMs":now+300_000},"policy":{"id":"fixture-policy","revision":"r1"},"sessionRequirement":{"kind":"notRequired"}
    });
    let spec =
        serde_json::from_value(spec).map_err(|_| error("fixture", "测试执行输入结构错误"))?;
    FrozenExecution::freeze(spec, &INPUT_LIMITS)
        .map_err(|_| error("fixture", "测试执行输入校验失败"))
}

/// Protected S1 classification after exact fixture reconstruction, never tool-description inference.
pub fn risk(resource: &str) -> Option<execution_admission::RiskLevel> {
    use execution_admission::RiskLevel::*;
    match resource {
        "fixture-parameter-check" => Some(Zero),
        "fixture-diagnostics" | "fixture-unknown" => Some(One),
        "fixture-office" | "fixture-long-running" => Some(Two),
        "fixture-blocked" | "fixture-unsupported" | "fixture-withdrawn" => Some(Three),
        _ => None,
    }
}
