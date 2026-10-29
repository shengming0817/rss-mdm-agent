import { proxy } from "valtio/vanilla";
import { deepClone } from "valtio/vanilla/utils";
import type { ServicePort } from "../settings/native";
import type {
  ServiceView,
  RegistrationStatus,
  MdmStatus,
} from "../settings/service-contract";
/** Independent original observations; a successful read never upgrades a registration or MDM phase. */
export function createRegistration(port?: ServicePort) {
  const state = proxy({
    service: undefined as ServiceView | undefined,
    registration: undefined as RegistrationStatus | undefined,
    mdm: undefined as MdmStatus | undefined,
    loading: false,
    serviceError: "",
    registrationError: "",
    mdmError: "",
  });
  let active = true,
    epoch = 0;
  return {
    state,
    async refresh() {
      if (!active || !port || state.loading) return;
      const current = ++epoch;
      state.loading = true;
      const [service, registration, mdm] = await Promise.allSettled([
        Promise.resolve().then(() => port.read()),
        Promise.resolve().then(() => port.registration()),
        Promise.resolve().then(() => port.mdmStatus()),
      ]);
      if (!active || current !== epoch) return;
      if (service.status === "fulfilled") {
        state.service = deepClone(service.value);
        state.serviceError = "";
      } else {
        state.service = undefined;
        state.serviceError = "无法读取本机服务状态";
      }
      if (registration.status === "fulfilled") {
        state.registration = deepClone(registration.value);
        state.registrationError = "";
      } else {
        state.registration = undefined;
        state.registrationError = "设备注册状态尚未确认";
      }
      if (mdm.status === "fulfilled") {
        state.mdm = deepClone(mdm.value);
        state.mdmError = "";
      } else {
        state.mdm = undefined;
        state.mdmError = "MDM 管理状态尚未确认";
      }
      state.loading = false;
    },
    dispose() {
      active = false;
      epoch++;
      state.service = undefined;
      state.registration = undefined;
      state.mdm = undefined;
      state.loading = false;
    },
  };
}
export const registrationLabels: Record<RegistrationStatus["phase"], string> = {
  unregistered: "尚未注册",
  preparing: "正在准备注册",
  configurationUnknown: "注册配置待核实",
  awaitingAuthorization: "等待组织授权",
  authorized: "组织已授权，待激活",
  activating: "正在激活",
  active: "已激活",
  issuanceUnknown: "签发结果未知",
  activationUnknown: "激活结果未知",
  paused: "注册已暂停",
  rejected: "注册未获批准",
};
