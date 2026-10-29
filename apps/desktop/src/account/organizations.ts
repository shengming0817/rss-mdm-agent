import { proxy } from "valtio/vanilla";
import { deepClone } from "valtio/vanilla/utils";
import type {
  AccountOrganization,
  AccountSettings,
} from "@rss-mdm-agent/ai-contract/wire";
import {
  accountErrorMessage,
  loadAccountSettings,
  saveOrganization,
} from "../test-users";
export function createOrganizations(
  port: {
    load(): Promise<AccountSettings>;
    save(input: AccountOrganization): Promise<AccountOrganization>;
  } = { load: loadAccountSettings, save: saveOrganization },
) {
  const state = proxy({
    items: [] as AccountOrganization[],
    selected: "",
    loading: false,
    saving: false,
    error: "",
  });
  let active = true,
    epoch = 0;
  async function load() {
    if (!active || state.loading) return;
    const current = ++epoch;
    state.loading = true;
    state.error = "";
    try {
      const page = await port.load();
      if (active && current === epoch) {
        state.items = deepClone(page.organizations);
        state.selected = page.selected ?? page.organizations[0]?.id ?? "";
      }
    } catch (error) {
      if (active && current === epoch) state.error = accountErrorMessage(error);
    } finally {
      if (active && current === epoch) state.loading = false;
    }
  }
  return {
    state,
    load,
    async save(input: AccountOrganization) {
      if (!active || state.saving) return false;
      state.saving = true;
      state.error = "";
      const current = ++epoch;
      try {
        await port.save(input);
        if (!active || current !== epoch) return false;
        state.loading = false;
        await load();
        return !state.error;
      } catch (error) {
        if (active && current === epoch)
          state.error = accountErrorMessage(error);
        return false;
      } finally {
        if (active) state.saving = false;
      }
    },
    dispose() {
      active = false;
      epoch++;
      state.items = [];
      state.selected = "";
      state.loading = false;
      state.saving = false;
      state.error = "";
    },
  };
}
