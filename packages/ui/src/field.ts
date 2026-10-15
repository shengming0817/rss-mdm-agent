import {
  computed,
  inject,
  onBeforeUnmount,
  type InjectionKey,
  type Ref,
} from "vue";
import type { UiFocusable } from "./context";
export interface FieldContext {
  id: string;
  descriptionId: Ref<string | undefined>;
  invalid: Ref<boolean>;
  register(control: UiFocusable): () => void;
}
export const fieldContext: InjectionKey<FieldContext> = Symbol("ui-field");
export function useFieldControl(focus: () => void) {
  const field = inject(fieldContext, undefined);
  const unregister = field?.register({ focus });
  onBeforeUnmount(() => unregister?.());
  return computed(() => ({
    id: field?.id,
    "aria-describedby": field?.descriptionId.value,
    "aria-invalid": field?.invalid.value || undefined,
  }));
}
